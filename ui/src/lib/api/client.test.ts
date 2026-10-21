import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const goto = vi.fn();
vi.mock('$app/navigation', () => ({ goto }));

import { ApiError, authorizedFetch, checkSignIn, get, logout } from './client.js';

function store(): Storage {
	const data = new Map<string, string>();
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

/** What a browser's fetch hands back for a redirect it was told not to follow. */
const opaqueRedirect = () => ({ type: 'opaqueredirect', status: 0, ok: false }) as Response;
const json = (status: number, body: unknown) =>
	new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

let fetchMock: ReturnType<typeof vi.fn>;
let reload: ReturnType<typeof vi.fn>;

beforeEach(() => {
	fetchMock = vi.fn();
	reload = vi.fn();
	vi.stubGlobal('fetch', fetchMock);
	vi.stubGlobal('window', { location: { reload, href: '' } });
	vi.stubGlobal('localStorage', store());
	vi.stubGlobal('sessionStorage', store());
	goto.mockClear();
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('the web app behind a login proxy', () => {
	it('asks fetch not to follow redirects', async () => {
		fetchMock.mockResolvedValue(json(200, { ok: true }));
		await get('/servers');
		expect(fetchMock.mock.calls[0][1]).toMatchObject({
			redirect: 'manual',
			credentials: 'include'
		});
	});

	it('reloads the page when the proxy redirects an API call, once per half minute', async () => {
		vi.useFakeTimers();
		fetchMock.mockResolvedValue(opaqueRedirect());

		await expect(get('/servers')).rejects.toBeInstanceOf(ApiError);
		expect(reload).toHaveBeenCalledTimes(1);
		expect(goto).not.toHaveBeenCalled();

		// A proxy that keeps redirecting cannot loop the page
		await expect(authorizedFetch('/servers/1/files')).rejects.toBeInstanceOf(ApiError);
		expect(reload).toHaveBeenCalledTimes(1);

		vi.advanceTimersByTime(31_000);
		await expect(get('/servers')).rejects.toBeInstanceOf(ApiError);
		expect(reload).toHaveBeenCalledTimes(2);
	});

	it("sends the app's own 401 to the server's login page, not the SPA's", async () => {
		fetchMock.mockResolvedValue(json(401, { error: 'Authentication required' }));
		await expect(get('/servers')).rejects.toMatchObject({ status: 401 });
		// A full page load: the server's page offers the identity provider and two-factor
		expect(window.location.href).toBe('/auth/login');
		expect(goto).not.toHaveBeenCalled();
		expect(reload).not.toHaveBeenCalled();
	});

	it('signs out to the login page without going straight back to the provider', async () => {
		fetchMock.mockResolvedValue(new Response(null, { status: 200 }));
		await logout();
		expect(fetchMock.mock.calls[0][0]).toBe('/auth/logout');
		expect(fetchMock.mock.calls[0][1]).toMatchObject({ method: 'POST' });
		expect(window.location.href).toBe('/auth/login?signed_out=1');
		expect(goto).not.toHaveBeenCalled();
	});
});

describe('checkSignIn', () => {
	it('handles a redirect like any call', async () => {
		fetchMock.mockResolvedValue(opaqueRedirect());
		await checkSignIn();
		expect(fetchMock.mock.calls[0][0]).toBe('/api/v1/sessions');
		expect(reload).toHaveBeenCalledTimes(1);
	});

	it('stays quiet when the server is unreachable or still signed in', async () => {
		fetchMock.mockRejectedValueOnce(new TypeError('Failed to fetch'));
		await expect(checkSignIn()).resolves.toBeUndefined();
		fetchMock.mockResolvedValueOnce(json(200, { sessions: [], detach_ttl_secs: 900 }));
		await expect(checkSignIn()).resolves.toBeUndefined();
		expect(reload).not.toHaveBeenCalled();
		expect(goto).not.toHaveBeenCalled();
	});
});
