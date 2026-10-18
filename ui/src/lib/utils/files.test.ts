import { describe, expect, it } from 'vitest';
import { baseName, breadcrumbs, formatSize, joinPath, parentPath, validName } from './files.js';

describe('remote path helpers', () => {
	it('builds breadcrumbs from the root', () => {
		expect(breadcrumbs('/home/ubuntu/src')).toEqual([
			{ label: '/', path: '/' },
			{ label: 'home', path: '/home' },
			{ label: 'ubuntu', path: '/home/ubuntu' },
			{ label: 'src', path: '/home/ubuntu/src' }
		]);
		expect(breadcrumbs('/')).toEqual([{ label: '/', path: '/' }]);
	});

	it('joins, splits and names', () => {
		expect(joinPath('/', 'etc')).toBe('/etc');
		expect(joinPath('/home', 'ubuntu')).toBe('/home/ubuntu');
		expect(parentPath('/')).toBeNull();
		expect(parentPath('/home')).toBe('/');
		expect(parentPath('/home/ubuntu/')).toBe('/home');
		expect(baseName('/home/ubuntu/notes.txt')).toBe('notes.txt');
		expect(baseName('/')).toBe('/');
	});

	it('rejects names that are not a single path segment', () => {
		expect(validName('notes.txt')).toBeNull();
		expect(validName('  ')).toMatch(/required/);
		expect(validName('..')).toMatch(/reserved/);
		expect(validName('a/b')).toMatch(/\//);
	});

	it('formats sizes', () => {
		expect(formatSize(0)).toBe('0 B');
		expect(formatSize(1023)).toBe('1023 B');
		expect(formatSize(1536)).toBe('1.5 KB');
		expect(formatSize(340 * 1024 * 1024)).toBe('340 MB');
		expect(formatSize(-1)).toBe('');
	});
});
