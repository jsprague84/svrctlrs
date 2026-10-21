import { get, post, del, authorizedFetch } from './client.js';

/** One row of a remote directory listing, as `GET /servers/{id}/files` returns it. */
export interface RemoteEntry {
	name: string;
	kind: 'dir' | 'file' | 'symlink' | 'other';
	size: number;
	modified: string | null;
	mode: string;
	owner: string | null;
	group: string | null;
}

export interface Listing {
	server_id: number;
	/** Canonical absolute path of the directory listed */
	path: string;
	parent: string | null;
	entries: RemoteEntry[];
}

const base = (serverId: number) => `/servers/${serverId}/files`;
const q = (path: string) => `?path=${encodeURIComponent(path)}`;

/** Empty path lists the login's home directory. */
export function listFiles(serverId: number, path = ''): Promise<Listing> {
	return get<Listing>(`${base(serverId)}${q(path)}`);
}

export function makeDirectory(serverId: number, path: string): Promise<void> {
	return post(`${base(serverId)}/mkdir`, { path });
}

export function renamePath(serverId: number, from: string, to: string): Promise<void> {
	return post(`${base(serverId)}/rename`, { from, to });
}

/** Deletes a file or an empty directory. */
export function removePath(serverId: number, path: string): Promise<void> {
	return del(`${base(serverId)}${q(path)}`);
}

/** Fetch a file as a Blob (with the session's credentials; a plain link could not carry the
 *  native apps' token). The caller hands it to the browser's save dialog. */
export async function downloadFile(serverId: number, path: string): Promise<Blob> {
	const res = await authorizedFetch(`${base(serverId)}/content${q(path)}`);
	return res.blob();
}

/** A file read for the viewer: its bytes, and the server's tag when it is small enough to
 *  edit (sent back with a save so a file changed in between is not overwritten). */
export interface OpenedFile {
	blob: Blob;
	etag: string | null;
}

export class FileTooLarge extends Error {
	constructor(public size: number) {
		super('Too large to show');
		this.name = 'FileTooLarge';
	}
}

/** Read a file to look at it (audited as a view). Refuses one larger than `maxBytes` from its
 *  length before reading the body, so a symlink to a big file costs nothing. */
export async function openFile(
	serverId: number,
	path: string,
	maxBytes: number
): Promise<OpenedFile> {
	const res = await authorizedFetch(`${base(serverId)}/content${q(path)}&view=true`);
	const length = Number(res.headers.get('content-length') ?? NaN);
	if (Number.isFinite(length) && length > maxBytes) {
		void res.body?.cancel();
		throw new FileTooLarge(length);
	}
	const blob = await res.blob();
	if (blob.size > maxBytes) throw new FileTooLarge(blob.size);
	return { blob, etag: res.headers.get('etag') };
}

/** Save an edit: written only if the file still has `etag` (else an `ApiError` 412). Returns
 *  the new tag. */
export async function saveFile(
	serverId: number,
	path: string,
	bytes: Uint8Array,
	etag: string
): Promise<string> {
	const res = await authorizedFetch(`${base(serverId)}/content${q(path)}`, {
		method: 'PUT',
		body: new Blob([bytes as BlobPart]),
		headers: { 'Content-Type': 'application/octet-stream', 'If-Match': etag }
	});
	const saved = (await res.json()) as { etag: string };
	return saved.etag;
}

/** Stream a File to the server; creates or overwrites `path`. */
export async function uploadFile(serverId: number, path: string, file: Blob): Promise<void> {
	await authorizedFetch(`${base(serverId)}/content${q(path)}`, {
		method: 'PUT',
		body: file,
		headers: { 'Content-Type': 'application/octet-stream' }
	});
}
