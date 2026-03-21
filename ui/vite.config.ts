import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		proxy: {
			'/api': 'http://localhost:8080',
			'/ws': {
				target: 'http://localhost:8080',
				ws: true
			}
		},
		// Disable HMR WebSocket when running inside Tauri (crashes WebKitGTK)
		// @ts-ignore -- process.env exists at build time in Node
		hmr: !!process.env.TAURI_ENV_PLATFORM ? false : undefined
	}
});
