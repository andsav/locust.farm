import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	server: { proxy: { '/api': process.env.LOCUST_FARM_API ?? 'http://127.0.0.1:4319' } },
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},

			adapter: adapter({ fallback: 'farm.html' }),
			csp: {
				mode: 'hash',
				directives: {
					'default-src': ['self'],
					'script-src': ['self'],
					'style-src': ['self', 'unsafe-inline'],
					'connect-src': ['self'],
					'img-src': ['self', 'data:'],
					'font-src': ['self'],
					'object-src': ['none'],
					'base-uri': ['self']
				}
			},

			// Inline all CSS into the prerendered HTML, so first paint needs no stylesheet request.
			inlineStyleThreshold: Infinity
		})
	]
});
