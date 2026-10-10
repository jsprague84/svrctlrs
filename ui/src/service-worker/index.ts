// Makes the web app installable on a phone (standalone window, no browser chrome) and serves
// the app shell's hashed assets from a cache. Nothing dynamic is cached: `/api/`, `/ws/` and
// page navigations always go to the network, so a stale worker can never show stale data or
// bypass the edge login. Each build gets its own cache; older ones are deleted on activate.

import { version } from '$app/env';
import { assets, immutable } from '$app/manifest';
import { self as sw } from '$app/service-worker';

const CACHE = `svrctlrs-${version}`;
// Manifest entries are paths relative to the app's base; resolve them against the worker's own
// URL so the set matches request pathnames whatever the base is.
const ASSETS = new Set<string>(
	[...immutable, ...assets].map((a) => new URL(a.path, sw.location.href).pathname)
);

sw.addEventListener('install', (event) => {
	event.waitUntil(
		caches
			.open(CACHE)
			.then((cache) => cache.addAll([...ASSETS]))
			.then(() => sw.skipWaiting())
	);
});

sw.addEventListener('activate', (event) => {
	event.waitUntil(
		caches
			.keys()
			.then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
			.then(() => sw.clients.claim())
	);
});

sw.addEventListener('fetch', (event) => {
	if (event.request.method !== 'GET') return;
	const url = new URL(event.request.url);
	if (url.origin !== sw.location.origin || !ASSETS.has(url.pathname)) return;

	event.respondWith(
		caches.open(CACHE).then(async (cache) => {
			const hit = await cache.match(event.request);
			if (hit) return hit;
			const response = await fetch(event.request);
			if (response.ok) cache.put(event.request, response.clone());
			return response;
		})
	);
});
