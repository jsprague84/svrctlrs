import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
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
