import { describe, expect, it } from 'vitest';
import type { Server } from '#lib/types/index.js';
import { serverStatus } from './serverStatus.js';

const base: Server = {
	id: 1,
	name: 'web',
	hostname: 'web.example',
	port: 22,
	username: 'root',
	credential_id: null,
	description: null,
	is_local: false,
	enabled: true,
	os_type: null,
	os_distro: null,
	package_manager: null,
	docker_available: false,
	systemd_available: false,
	metadata: null,
	last_seen_at: null,
	last_error: null,
	created_at: '2026-10-01T00:00:00Z',
	updated_at: '2026-10-01T00:00:00Z'
};
const now = new Date('2026-10-10T12:00:00Z');

describe('serverStatus', () => {
	it('never claims online without evidence', () => {
		expect(serverStatus(base, now).kind).toBe('unknown');
	});
	it('reports a recorded failure', () => {
		const s = serverStatus({ ...base, last_error: 'Unreachable on web.example:22' }, now);
		expect(s.kind).toBe('unreachable');
		expect(s.dot).toBe('bg-error');
	});
	it('is online only for a recent success', () => {
		expect(serverStatus({ ...base, last_seen_at: '2026-10-10T11:50:00Z' }, now).kind).toBe(
			'online'
		);
		expect(serverStatus({ ...base, last_seen_at: '2026-10-10T09:00:00Z' }, now).kind).toBe(
			'unknown'
		);
	});
	it('disabled and local are stated as such', () => {
		expect(serverStatus({ ...base, enabled: false, last_error: 'x' }, now).kind).toBe('disabled');
		expect(serverStatus({ ...base, is_local: true }, now).kind).toBe('online');
	});
});
