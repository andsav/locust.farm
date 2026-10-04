// Browser tests for the formation editor, run against the production build.
// npm run test:e2e builds the site, serves it with vite preview and runs them.
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
	testDir: 'e2e',
	timeout: 30_000,
	fullyParallel: true,
	reporter: [['list']],
	use: {
		baseURL: 'http://localhost:4174',
		...devices['Desktop Chrome'],
		viewport: { width: 1440, height: 900 },
		permissions: ['clipboard-read', 'clipboard-write']
	},
	webServer: {
		command: 'npm run preview -- --port 4174 --strictPort',
		url: 'http://localhost:4174/formations',
		reuseExistingServer: false,
		timeout: 60_000
	}
});
