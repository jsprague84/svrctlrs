import type { Listing, RemoteEntry } from '#lib/api/files.js';
import * as api from '#lib/api/files.js';
import { extractErrorMessage } from '#lib/utils/error.js';
import { baseName, joinPath } from '#lib/utils/files.js';
import { SvelteMap } from 'svelte/reactivity';
import { saveBlob } from '#lib/utils/download.js';
import {
	EDIT_MAX,
	VIEW_IMAGE_MAX,
	VIEW_TEXT_MAX,
	decodeText,
	encodeText,
	imageType,
	viewKind,
	type DecodedText,
	type ViewKind
} from '#lib/utils/fileView.js';

/**
 * The remote file panel: one server at a time, the directory it is showing, and the last path
 * visited on each server so switching tabs comes back to where you were.
 */
let serverId = $state<number | null>(null);
let serverName = $state<string | null>(null);
let listing = $state<Listing | null>(null);
let loading = $state(false);
let error = $state<string | null>(null);
/** Something in flight that is not a listing: upload, delete, rename, mkdir */
let busy = $state<string | null>(null);
const lastPath = new SvelteMap<number, string>();

export function getServerId() {
	return serverId;
}
export function getServerName() {
	return serverName;
}
export function getListing() {
	return listing;
}
export function isLoading() {
	return loading;
}
export function getError() {
	return error;
}
export function getBusy() {
	return busy;
}

/** Show a directory; with no path, the last one visited on this server (or home). False when
 *  it could not be listed (the error is shown). */
export async function browse(id: number, name: string | null, path?: string): Promise<boolean> {
	if (serverId !== id) listing = null;
	serverId = id;
	serverName = name;
	const target = path ?? lastPath.get(id) ?? '';
	loading = true;
	error = null;
	try {
		const result = await api.listFiles(id, target);
		if (serverId !== id) return false; // switched away while loading
		listing = result;
		lastPath.set(id, result.path);
		return true;
	} catch (e) {
		if (serverId !== id) return false;
		error = extractErrorMessage(e, 'Could not list the directory');
		return false;
	} finally {
		if (serverId === id) loading = false;
	}
}

export function clearError() {
	error = null;
}

export async function refresh() {
	if (serverId === null) return;
	await browse(serverId, serverName, listing?.path ?? lastPath.get(serverId) ?? '');
}

function current(): { id: number; dir: string } {
	if (serverId === null || !listing) throw new Error('No directory is open');
	return { id: serverId, dir: listing.path };
}

async function withBusy<T>(label: string, work: () => Promise<T>): Promise<T> {
	busy = label;
	try {
		return await work();
	} finally {
		busy = null;
	}
}

export async function makeDirectory(name: string): Promise<void> {
	const { id, dir } = current();
	await withBusy(`Creating ${name}`, () => api.makeDirectory(id, joinPath(dir, name)));
	await refresh();
}

export async function rename(entry: RemoteEntry, newName: string): Promise<void> {
	const { id, dir } = current();
	await withBusy(`Renaming ${entry.name}`, () =>
		api.renamePath(id, joinPath(dir, entry.name), joinPath(dir, newName))
	);
	await refresh();
}

export async function remove(entry: RemoteEntry): Promise<void> {
	const { id, dir } = current();
	await withBusy(`Deleting ${entry.name}`, () => api.removePath(id, joinPath(dir, entry.name)));
	await refresh();
}

export async function upload(file: File): Promise<void> {
	const { id, dir } = current();
	await withBusy(`Uploading ${file.name}`, () =>
		api.uploadFile(id, joinPath(dir, file.name), file)
	);
	await refresh();
}

/** Fetch the file and hand it to the browser's save dialog. */
export async function download(entry: RemoteEntry): Promise<void> {
	const { id, dir } = current();
	const path = joinPath(dir, entry.name);
	const blob = await withBusy(`Downloading ${entry.name}`, () => api.downloadFile(id, path));
	saveBlob(blob, baseName(path));
}

// ── The viewer ───────────────────────────────────────────────────────────

/** The file open in the viewer: what it is, and what was read of it */
export interface Viewing {
	serverId: number;
	serverName: string | null;
	path: string;
	name: string;
	/** From the listing; the read decides for a symlink */
	entry: RemoteEntry;
	kind: ViewKind;
	loading: boolean;
	error: string | null;
	blob: Blob | null;
	/** An object URL for an image; revoked when the viewer closes */
	imageUrl: string | null;
	decoded: DecodedText | null;
	/** The server's tag: present for files small enough to edit */
	etag: string | null;
}

let viewing = $state<Viewing | null>(null);

export function getViewing() {
	return viewing;
}

function revokeImage() {
	if (viewing?.imageUrl) URL.revokeObjectURL(viewing.imageUrl);
}

/** Open a file of the current directory in the viewer and read it, when it is small enough. */
export async function view(entry: RemoteEntry): Promise<void> {
	const { id, dir } = current();
	revokeImage();
	const path = joinPath(dir, entry.name);
	viewing = {
		serverId: id,
		serverName,
		path,
		name: entry.name,
		entry,
		kind: viewKind(entry),
		loading: false,
		error: null,
		blob: null,
		imageUrl: null,
		decoded: null,
		etag: null
	};
	await load();
}

/** (Re)read the open file. */
export async function load(): Promise<void> {
	const v = viewing;
	if (!v || v.kind === 'too-large' || v.kind === 'special') return;
	v.loading = true;
	v.error = null;
	try {
		const image = v.kind === 'image' ? imageType(v.name) : null;
		const opened = await api.openFile(v.serverId, v.path, image ? VIEW_IMAGE_MAX : VIEW_TEXT_MAX);
		if (viewing !== v) return;
		v.blob = opened.blob;
		v.etag = opened.etag;
		if (image) {
			revokeImage();
			v.imageUrl = URL.createObjectURL(new Blob([opened.blob], { type: image }));
		} else {
			v.decoded = decodeText(new Uint8Array(await opened.blob.arrayBuffer()));
		}
	} catch (e) {
		if (viewing !== v) return;
		if (e instanceof api.FileTooLarge) {
			v.kind = 'too-large';
			v.entry = { ...v.entry, size: e.size };
		} else {
			v.error = extractErrorMessage(e, `Could not open ${v.name}`);
		}
	} finally {
		if (viewing === v) v.loading = false;
	}
}

export function closeViewer() {
	revokeImage();
	viewing = null;
}

/** Save the editor's text over the open file, if it is unchanged since it was read; the
 *  server's refusal (412 changed, 403 not allowed) is thrown for the editor to show. */
export async function saveText(text: string): Promise<void> {
	const v = viewing;
	if (!v?.decoded || !v.etag) throw new Error('This file cannot be edited here');
	const bytes = encodeText(text, v.decoded);
	if (bytes.length > EDIT_MAX) {
		throw new Error(`Too large to save from here (over ${EDIT_MAX / 1024} KB)`);
	}
	const etag = await api.saveFile(v.serverId, v.path, bytes, v.etag);
	if (viewing !== v) return;
	v.etag = etag;
	v.blob = new Blob([bytes as BlobPart]);
	v.decoded = { ...v.decoded, text: text.replace(/\r\n?/g, '\n') };
	if (serverId === v.serverId) void refresh();
}

/** Save the open file locally: the bytes already read, or a download when it was too big. */
export async function downloadViewed(): Promise<void> {
	const v = viewing;
	if (!v) return;
	if (v.blob) {
		saveBlob(v.blob, v.name);
		return;
	}
	const blob = await withBusy(`Downloading ${v.name}`, () => api.downloadFile(v.serverId, v.path));
	saveBlob(blob, v.name);
}
