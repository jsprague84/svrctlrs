import { describe, expect, it } from 'vitest';
import { normalizeServerUrl } from './url.js';

describe('normalizeServerUrl', () => {
	it('defaults to https and strips trailing slashes', () => {
		expect(normalizeServerUrl('  svrctlrs.example.com/ ', 'tauri-desktop')).toEqual({
			url: 'https://svrctlrs.example.com'
		});
		expect(normalizeServerUrl('https://host:8443/base/', 'web')).toEqual({
			url: 'https://host:8443/base'
		});
	});

	it('rejects empty, malformed, and non-http schemes', () => {
		expect(normalizeServerUrl('', 'web')).toHaveProperty('error');
		expect(normalizeServerUrl('ftp://x', 'web')).toHaveProperty('error');
		expect(normalizeServerUrl('http://', 'web')).toHaveProperty('error');
	});

	it('allows plain http on desktop but not on Android', () => {
		expect(normalizeServerUrl('http://10.0.0.5:8080', 'tauri-desktop')).toEqual({
			url: 'http://10.0.0.5:8080'
		});
		const mobile = normalizeServerUrl('http://10.0.0.5:8080', 'tauri-mobile');
		expect('error' in mobile && mobile.error).toMatch(/https/);
	});

	it('refuses credentials in the URL', () => {
		expect(normalizeServerUrl('https://user:pw@host', 'web')).toHaveProperty('error');
	});
});
