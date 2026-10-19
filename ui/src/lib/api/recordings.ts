import { get, del, authorizedFetch } from './client.js';
import type { RecordingsListResponse } from '#lib/types/index.js';

export interface ListOptions {
	serverId?: number;
	/** The smallest id of the previous page */
	before?: number;
	limit?: number;
}

/** The calling user's recordings, newest first. */
export function listRecordings(opts: ListOptions = {}): Promise<RecordingsListResponse> {
	const q = new URLSearchParams();
	if (opts.serverId !== undefined) q.set('server_id', String(opts.serverId));
	if (opts.before !== undefined) q.set('before', String(opts.before));
	if (opts.limit !== undefined) q.set('limit', String(opts.limit));
	const qs = q.toString();
	return get<RecordingsListResponse>(`/recordings${qs ? `?${qs}` : ''}`);
}

/** The recording as asciicast v2 text, for the player. */
export async function fetchCast(id: number): Promise<string> {
	const res = await authorizedFetch(`/recordings/${id}/cast`);
	return res.text();
}

/** The recording as a file to save (audited as a download). A plain link could not carry the
 *  native apps' token, so it is fetched and handed to the save dialog. */
export async function downloadCast(id: number): Promise<Blob> {
	const res = await authorizedFetch(`/recordings/${id}/cast?download=true`);
	return res.blob();
}

/** 409 while the shell is still being recorded. */
export async function deleteRecording(id: number): Promise<void> {
	await del(`/recordings/${id}`);
}
