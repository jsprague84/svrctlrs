import type { Server } from '#lib/types/index.js';

/** Every tag in use, each once, sorted ignoring case. */
export function allTags(servers: Server[]): string[] {
	const seen = new Map<string, string>();
	for (const s of servers) {
		for (const t of s.tags ?? []) {
			const key = t.toLowerCase();
			if (!seen.has(key)) seen.set(key, t);
		}
	}
	return [...seen.values()].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));
}

/** `null` matches everything; a tag matches ignoring case. */
export function matchesTag(server: Server, tag: string | null): boolean {
	if (tag === null) return true;
	const want = tag.toLowerCase();
	return (server.tags ?? []).some((t) => t.toLowerCase() === want);
}

/** "prod, home,db" → ["prod", "home", "db"] (trimmed, empties dropped) */
export function parseTagInput(text: string): string[] {
	return text
		.split(',')
		.map((t) => t.trim())
		.filter((t) => t.length > 0);
}
