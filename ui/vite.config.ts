import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { readFileSync } from 'node:fs';
import { defineConfig } from 'vite';

// Single source of truth for the UI version: package.json, kept in step with the Cargo
// workspace and tauri.conf.json (see docs/DEVELOPMENT.md, "Releasing").
const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8'));

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	define: {
		__APP_VERSION__: JSON.stringify(pkg.version)
	},
	server: {
		// Bind to all interfaces when TAURI_DEV_HOST is set (needed for mobile dev)
		host: process.env.TAURI_DEV_HOST || 'localhost',
		proxy: {
			'/api': 'http://localhost:8080',
			'/ws': {
				target: 'http://localhost:8080',
				ws: true
			}
		}
	}
});
