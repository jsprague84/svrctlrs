import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

function store(initial: Record<string, string> = {}): Storage {
	const data = new Map(Object.entries(initial));
	return {
		get length() {
			return data.size;
		},
		clear: () => data.clear(),
		getItem: (k) => data.get(k) ?? null,
		key: (i) => [...data.keys()][i] ?? null,
		removeItem: (k) => void data.delete(k),
		setItem: (k, v) => void data.set(k, String(v))
	};
}

async function fresh() {
	vi.resetModules();
	return import('./appToken.js');
}

beforeEach(() => {
	invoke.mockReset();
	vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
	vi.stubGlobal('navigator', { userAgent: 'Linux' });
});

afterEach(() => vi.unstubAllGlobals());

describe('the desktop app token', () => {
	it('moves a token an older build left in localStorage into the keyring', async () => {
		const ls = store({
			'svrctlrs-server-url': 'https://svr.example',
			'svrctlrs-session-token': 'old-token'
		});
		vi.stubGlobal('localStorage', ls);
		invoke.mockResolvedValue(undefined);
		const t = await fresh();
		await t.loadAppToken();
		expect(t.getAppToken()).toBe('old-token');
		expect(ls.getItem('svrctlrs-session-token')).toBeNull();
		expect(invoke).toHaveBeenCalledWith('store_token', {
			serverUrl: 'https://svr.example',
			token: 'old-token'
		});
	});

	it('reads the keyring for this server, and forgets on sign-out', async () => {
		vi.stubGlobal('localStorage', store({ 'svrctlrs-server-url': 'https://svr.example' }));
		invoke.mockImplementation(async (cmd: string) => (cmd === 'stored_token' ? 'kept' : undefined));
		const t = await fresh();
		await t.loadAppToken();
		expect(t.getAppToken()).toBe('kept');
		await t.forgetAppToken();
		expect(t.getAppToken()).toBeNull();
		expect(invoke).toHaveBeenCalledWith('forget_token', { serverUrl: 'https://svr.example' });
	});

	it('works without a keyring: signed out, not broken', async () => {
		vi.stubGlobal('localStorage', store({ 'svrctlrs-server-url': 'https://svr.example' }));
		invoke.mockRejectedValue(new Error('no secret service'));
		const t = await fresh();
		await t.loadAppToken();
		expect(t.getAppToken()).toBeNull();
	});

	it('does nothing in the browser', async () => {
		vi.stubGlobal('window', {});
		vi.stubGlobal('localStorage', store({ 'svrctlrs-session-token': 'x' }));
		const t = await fresh();
		await t.loadAppToken();
		expect(t.getAppToken()).toBeNull();
		expect(invoke).not.toHaveBeenCalled();
	});
});
