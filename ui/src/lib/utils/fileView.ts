/**
 * What the file viewer does with a remote file: which files it shows and how, and the bytes
 * ↔ text round trip of the in-app editor (encoding, BOM, line endings kept as they were).
 */

/** Images shown in an `<img>` (an SVG there runs no script), by extension */
const IMAGE_TYPES: Record<string, string> = {
	png: 'image/png',
	jpg: 'image/jpeg',
	jpeg: 'image/jpeg',
	gif: 'image/gif',
	webp: 'image/webp',
	avif: 'image/avif',
	bmp: 'image/bmp',
	ico: 'image/x-icon',
	svg: 'image/svg+xml'
};

export const VIEW_IMAGE_MAX = 10 * 1024 * 1024;
export const VIEW_TEXT_MAX = 1024 * 1024;
/** The server tags files up to this size (`ETag`), which is what makes them editable */
export const EDIT_MAX = 256 * 1024;

export type ViewKind = 'image' | 'text' | 'too-large' | 'special';

export function imageType(name: string): string | null {
	const dot = name.lastIndexOf('.');
	if (dot <= 0) return null;
	return IMAGE_TYPES[name.slice(dot + 1).toLowerCase()] ?? null;
}

/** How the viewer opens an entry from a listing. `special` (a device, a pipe, a socket) is
 *  never read: a pipe would block. A symlink's size is the link's, so it is read with the
 *  text limit and the server's length decides. */
export function viewKind(entry: { name: string; kind: string; size: number }): ViewKind {
	if (entry.kind === 'other' || entry.kind === 'dir') return 'special';
	const known = entry.kind === 'file';
	if (imageType(entry.name)) {
		return known && entry.size > VIEW_IMAGE_MAX ? 'too-large' : 'image';
	}
	return known && entry.size > VIEW_TEXT_MAX ? 'too-large' : 'text';
}

export type LineEnding = 'lf' | 'crlf' | 'none' | 'mixed';

export interface DecodedText {
	/** A NUL byte near the start: not text, nothing to show */
	binary: boolean;
	/** Valid UTF-8; otherwise `text` has replacement characters and is read-only */
	utf8: boolean;
	bom: boolean;
	lineEnding: LineEnding;
	/** Line breaks as `\n` whatever the file uses, the way a textarea holds them */
	text: string;
}

const BOM = [0xef, 0xbb, 0xbf];

export function detectLineEnding(text: string): LineEnding {
	let crlf = 0;
	let lf = 0;
	let cr = 0;
	for (let i = 0; i < text.length; i++) {
		const c = text.charCodeAt(i);
		if (c === 13) {
			if (text.charCodeAt(i + 1) === 10) {
				crlf++;
				i++;
			} else cr++;
		} else if (c === 10) lf++;
	}
	// A lone CR cannot survive a textarea (it becomes a line break), so it counts as mixed
	if (cr > 0 || (crlf > 0 && lf > 0)) return 'mixed';
	if (crlf > 0) return 'crlf';
	if (lf > 0) return 'lf';
	return 'none';
}

export function decodeText(bytes: Uint8Array): DecodedText {
	if (bytes.subarray(0, 8192).includes(0)) {
		return { binary: true, utf8: false, bom: false, lineEnding: 'none', text: '' };
	}
	const bom = BOM.every((b, i) => bytes[i] === b);
	const body = bom ? bytes.subarray(3) : bytes;
	let raw: string;
	let utf8 = true;
	try {
		raw = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(body);
	} catch {
		utf8 = false;
		raw = new TextDecoder('utf-8', { ignoreBOM: true }).decode(body);
	}
	const lineEnding = detectLineEnding(raw);
	return { binary: false, utf8, bom, lineEnding, text: raw.replace(/\r\n/g, '\n') };
}

/** Why the in-app editor will not open this file, or null when it will. */
export function whyNotEditable(d: DecodedText, tagged: boolean): string | null {
	if (d.binary) return 'This is not a text file';
	if (!d.utf8) return 'Not UTF-8 text; edit it in the terminal';
	if (d.lineEnding === 'mixed') return 'Mixed line endings; edit it in the terminal';
	if (!tagged) return `Larger than ${EDIT_MAX / 1024} KB; edit it in the terminal`;
	return null;
}

/** The editor's text back to the file's bytes: its line endings and BOM as they were. */
export function encodeText(
	text: string,
	how: { bom: boolean; lineEnding: LineEnding }
): Uint8Array {
	let out = text.replace(/\r\n?/g, '\n');
	if (how.lineEnding === 'crlf') out = out.replace(/\n/g, '\r\n');
	const bytes = new TextEncoder().encode(out);
	if (!how.bom) return bytes;
	const withBom = new Uint8Array(bytes.length + 3);
	withBom.set(BOM);
	withBom.set(bytes, 3);
	return withBom;
}
