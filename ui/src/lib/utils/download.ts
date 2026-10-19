/** Hand a Blob to the browser's save dialog under `name`. */
export function saveBlob(blob: Blob, name: string): void {
	const url = URL.createObjectURL(blob);
	try {
		const a = document.createElement('a');
		a.href = url;
		a.download = name;
		a.rel = 'noopener';
		document.body.appendChild(a);
		a.click();
		a.remove();
	} finally {
		// Give the click a moment to start before the URL goes away
		setTimeout(() => URL.revokeObjectURL(url), 10_000);
	}
}
