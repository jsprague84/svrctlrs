import { describe, expect, it } from 'vitest';
import { allTags, matchesTag, parseTagInput } from './tags.js';
import type { Server } from '#lib/types/index.js';

function server(tags: string[]): Server {
	return {
		id: 1,
		name: 'x',
		hostname: 'x',
		port: 22,
		username: null,
		credential_id: null,
		via_server_id: null,
		tags,
		startup_command: null,
		description: null,
		enabled: true,
		last_seen_at: null,
		last_error: null,
		created_at: '',
		updated_at: ''
	};
}

describe('tags', () => {
	it('lists every tag once, ignoring case, sorted', () => {
		expect(allTags([server(['Prod', 'db']), server(['prod', 'Home'])])).toEqual([
			'db',
			'Home',
			'Prod'
		]);
		expect(allTags([])).toEqual([]);
	});
	it('matches ignoring case and null matches all', () => {
		expect(matchesTag(server(['Prod']), 'prod')).toBe(true);
		expect(matchesTag(server(['Prod']), 'db')).toBe(false);
		expect(matchesTag(server([]), null)).toBe(true);
	});
	it('parses comma-separated input', () => {
		expect(parseTagInput(' prod, home,,db ')).toEqual(['prod', 'home', 'db']);
		expect(parseTagInput('')).toEqual([]);
	});
});
