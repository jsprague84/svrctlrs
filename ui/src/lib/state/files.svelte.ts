import type { Listing, RemoteEntry } from '#lib/api/files.js';
import * as api from '#lib/api/files.js';
import { extractErrorMessage } from '#lib/utils/error.js';
import { baseName, joinPath } from '#lib/utils/files.js';
import { SvelteMap } from 'svelte/reactivity';

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

/** Show a directory; with no path, the last one visited on this server (or home). */
export async function browse(id: number, name: string | null, path?: string) {
	if (serverId !== id) listing = null;
	serverId = id;
	serverName = name;
	const target = path ?? lastPath.get(id) ?? '';
	loading = true;
	error = null;
	try {
		const result = await api.listFiles(id, target);
		if (serverId !== id) return; // switched away while loading
		listing = result;
		lastPath.set(id, result.path);
	} catch (e) {
		if (serverId !== id) return;
		error = extractErrorMessage(e, 'Could not list the directory');
	} finally {
		if (serverId === id) loading = false;
	}
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
	const url = URL.createObjectURL(blob);
	try {
		const a = document.createElement('a');
		a.href = url;
		a.download = baseName(path);
		a.rel = 'noopener';
		document.body.appendChild(a);
		a.click();
		a.remove();
	} finally {
		// Give the click a moment to start before the URL goes away
		setTimeout(() => URL.revokeObjectURL(url), 10_000);
	}
}
