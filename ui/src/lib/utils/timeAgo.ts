/** "just now", "5 min ago", "3 h ago", "2 d ago"; older dates fall back to the locale date. */
export function timeAgo(iso: string | Date, now: Date = new Date()): string {
	const then = iso instanceof Date ? iso : new Date(iso);
	const diff = now.getTime() - then.getTime();
	if (!Number.isFinite(diff)) return '';
	const s = Math.round(diff / 1000);
	if (s < 45) return 'just now';
	const m = Math.round(s / 60);
	if (m < 60) return `${m} min ago`;
	const h = Math.round(m / 60);
	if (h < 24) return `${h} h ago`;
	const d = Math.round(h / 24);
	if (d < 14) return `${d} d ago`;
	return then.toLocaleDateString();
}
