import { describe, expect, it } from 'vitest';
import { ptySocketUrl } from './socketUrl.js';

const page = { protocol: 'https:', host: 'svrctlrs.example' };

describe('ptySocketUrl', () => {
	it('uses the page origin and the cookie in the browser', async () => {
		const url = await ptySocketUrl({
			serverUrl: '',
			location: page,
			token: null,
			fetchTicket: () => Promise.reject(new Error('not called'))
		});
		expect(url).toBe('wss://svrctlrs.example/ws/terminal/pty');
	});

	it('uses ws: on a plain-http page', async () => {
		const url = await ptySocketUrl({
			serverUrl: '',
			location: { protocol: 'http:', host: 'localhost:5173' },
			token: null,
			fetchTicket: () => Promise.reject(new Error('not called'))
		});
		expect(url).toBe('ws://localhost:5173/ws/terminal/pty');
	});

	it('gives a native app a one-time ticket, never the token', async () => {
		const url = await ptySocketUrl({
			serverUrl: 'https://box.lan:8443',
			location: page,
			token: 'secret-token',
			fetchTicket: async () => 'tick/et'
		});
		expect(url).toBe('wss://box.lan:8443/ws/terminal/pty?ticket=tick%2Fet');
	});

	it('never puts the token in the URL, even when the ticket call fails', async () => {
		await expect(
			ptySocketUrl({
				serverUrl: 'http://box.lan',
				location: page,
				token: 'a b',
				fetchTicket: () => Promise.reject(new Error('404'))
			})
		).rejects.toThrow('404');
	});
});
