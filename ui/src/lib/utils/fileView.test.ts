import { describe, expect, it } from 'vitest';
import {
	EDIT_MAX,
	VIEW_IMAGE_MAX,
	VIEW_TEXT_MAX,
	decodeText,
	detectLineEnding,
	encodeText,
	imageType,
	viewKind,
	whyNotEditable
} from './fileView.js';

const bytes = (s: string) => new TextEncoder().encode(s);
const file = (name: string, size = 10, kind = 'file') => ({ name, size, kind });

describe('which files the viewer shows, and how', () => {
	it('shows images by extension, text otherwise, within the caps', () => {
		expect(imageType('logo.PNG')).toBe('image/png');
		expect(imageType('icon.svg')).toBe('image/svg+xml');
		expect(imageType('.png')).toBeNull();
		expect(imageType('Makefile')).toBeNull();
		expect(viewKind(file('a.jpg', VIEW_IMAGE_MAX))).toBe('image');
		expect(viewKind(file('a.jpg', VIEW_IMAGE_MAX + 1))).toBe('too-large');
		expect(viewKind(file('nginx.conf', VIEW_TEXT_MAX))).toBe('text');
		expect(viewKind(file('huge.log', VIEW_TEXT_MAX + 1))).toBe('too-large');
	});

	it('never reads a pipe or a device, and lets the server size a symlink', () => {
		expect(viewKind(file('fifo', 0, 'other'))).toBe('special');
		expect(viewKind(file('link', 999_999_999, 'symlink'))).toBe('text');
	});
});

describe('decoding', () => {
	it('keeps track of CRLF and a BOM, and hands the editor plain line breaks', () => {
		const d = decodeText(new Uint8Array([0xef, 0xbb, 0xbf, ...bytes('a\r\nb\r\n')]));
		expect(d).toEqual({ binary: false, utf8: true, bom: true, lineEnding: 'crlf', text: 'a\nb\n' });
		expect(whyNotEditable(d, true)).toBeNull();
	});

	it('calls a NUL byte binary and non-UTF-8 read-only', () => {
		expect(decodeText(new Uint8Array([0x7f, 0x45, 0x4c, 0x46, 0, 1])).binary).toBe(true);
		const latin1 = decodeText(new Uint8Array([0x63, 0x61, 0x66, 0xe9, 0x0a]));
		expect(latin1.utf8).toBe(false);
		expect(latin1.text).toBe('caf�\n');
		expect(whyNotEditable(latin1, true)).toMatch(/UTF-8/);
	});

	it('sees mixed endings, and a lone CR, as something only the terminal can keep', () => {
		expect(detectLineEnding('a\nb\r\n')).toBe('mixed');
		expect(detectLineEnding('progress\r100%\n')).toBe('mixed');
		expect(detectLineEnding('one line')).toBe('none');
		expect(detectLineEnding('a\nb')).toBe('lf');
		expect(whyNotEditable(decodeText(bytes('a\nb\r\n')), true)).toMatch(/Mixed/);
	});

	it('edits only what the server tagged', () => {
		expect(whyNotEditable(decodeText(bytes('x\n')), false)).toMatch(String(EDIT_MAX / 1024));
	});
});

describe('the round trip', () => {
	const roundTrip = (raw: Uint8Array) => {
		const d = decodeText(raw);
		return encodeText(d.text, d);
	};

	it('gives back the same bytes when nothing was changed', () => {
		for (const raw of [
			bytes('user nginx;\nworker_processes 2;\n'),
			bytes('a\r\nb\r\nno final newline'),
			bytes('ünïcödé ✓ 🦀\n\ttab\n'),
			bytes(''),
			new Uint8Array([0xef, 0xbb, 0xbf, ...bytes('[section]\r\nkey=1\r\n')])
		]) {
			expect(roundTrip(raw)).toEqual(raw);
		}
	});

	it('writes new lines the way the file already ends its lines', () => {
		expect(encodeText('a\nb\nnew\n', { bom: false, lineEnding: 'crlf' })).toEqual(
			bytes('a\r\nb\r\nnew\r\n')
		);
		expect(encodeText('a\r\nb', { bom: false, lineEnding: 'lf' })).toEqual(bytes('a\nb'));
		expect(encodeText('first\nsecond', { bom: false, lineEnding: 'none' })).toEqual(
			bytes('first\nsecond')
		);
	});
});
