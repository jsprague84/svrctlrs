/** Path and size helpers for the remote file panel. Remote paths are POSIX, always absolute. */

/** `/home/ubuntu/src` → `[['/', '/'], ['home', '/home'], ['ubuntu', '/home/ubuntu'], ['src', …]]` */
export function breadcrumbs(path: string): Array<{ label: string; path: string }> {
	const crumbs = [{ label: '/', path: '/' }];
	let acc = '';
	for (const seg of path.split('/').filter(Boolean)) {
		acc += `/${seg}`;
		crumbs.push({ label: seg, path: acc });
	}
	return crumbs;
}

export function joinPath(dir: string, name: string): string {
	return dir.endsWith('/') ? `${dir}${name}` : `${dir}/${name}`;
}

export function parentPath(path: string): string | null {
	const trimmed = path.replace(/\/+$/, '');
	if (!trimmed) return null;
	const i = trimmed.lastIndexOf('/');
	return i <= 0 ? '/' : trimmed.slice(0, i);
}

export function baseName(path: string): string {
	const trimmed = path.replace(/\/+$/, '');
	return trimmed.slice(trimmed.lastIndexOf('/') + 1) || '/';
}

/** A file or folder name as typed in a prompt: one path segment, no slashes, not `.`/`..` */
export function validName(name: string): string | null {
	const n = name.trim();
	if (!n) return 'Name is required';
	if (n === '.' || n === '..') return 'That name is reserved';
	if (n.includes('/')) return 'A name cannot contain /';
	if (n.includes('\0')) return 'Invalid character';
	return null;
}

/** "1.2 KB", "340 MB"; bytes below 1 KB are shown as a count. */
export function formatSize(bytes: number): string {
	if (!Number.isFinite(bytes) || bytes < 0) return '';
	if (bytes < 1024) return `${bytes} B`;
	const units = ['KB', 'MB', 'GB', 'TB'];
	let v = bytes / 1024;
	let i = 0;
	while (v >= 1024 && i < units.length - 1) {
		v /= 1024;
		i++;
	}
	return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}
