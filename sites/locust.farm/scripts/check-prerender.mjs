import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

const output = resolve('build');
const bytes = (path) => readFileSync(resolve(output, `.${path}`), 'utf8');
const pageFile = (path) => (path === '/' ? '/index.html' : `${path}.html`);
const index = JSON.parse(bytes('/docs/next/index.json'));
const publication = JSON.parse(bytes('/docs/next/reference/availability.json')).publication;
// Nginx serves these reviewed downloads outside the static website build.
// Their public bytes are checked by the release acceptance campaign.
const hostedDownloads = new Set([
	publication.terminalInstallerUrl,
	publication.installationGuideUrl,
	publication.releaseMetadataUrl,
	publication.downloadUrl
]);
const routes = [
	'/',
	'/start',
	'/formations',
	'/docs',
	...index.pages.map((page) => page.url),
	...index.routes.map((route) => route.url)
];
for (const route of routes) {
	const html = bytes(pageFile(route));
	assert.ok(html.includes('<h1'), `Missing article heading: ${route}`);
	for (const match of html.matchAll(/(?:src|href)="(\/_app\/[^"?#]+)[^"]*"/g))
		assert.ok(existsSync(resolve(output, `.${match[1]}`)), `Missing client asset: ${match[1]}`);
	for (const match of html.matchAll(/<a\b[^>]*href="([^"]+)"/g)) {
		const url = new URL(match[1], `https://locust.farm${route}`);
		if (url.origin !== 'https://locust.farm') continue;
		if (hostedDownloads.has(url.href)) continue;
		const target = url.pathname.includes('.') ? url.pathname : pageFile(url.pathname);
		assert.ok(
			existsSync(resolve(output, `.${target}`)),
			`Broken built link ${match[1]} in ${route}`
		);
		if (url.hash)
			assert.ok(
				bytes(target).includes(`id="${decodeURIComponent(url.hash.slice(1))}"`),
				`Broken built anchor ${match[1]} in ${route}`
			);
	}
}
for (const page of index.pages) {
	const raw = bytes(page.rawUrl);
	assert.equal(createHash('sha256').update(raw).digest('hex'), page.sha256, page.rawUrl);
	for (const heading of page.headings)
		assert.ok(bytes(pageFile(page.url)).includes(`id="${heading.id}"`));
}
for (const artifact of index.artifacts)
	assert.equal(
		createHash('sha256').update(bytes(artifact.url)).digest('hex'),
		artifact.sha256,
		artifact.url
	);
assert.ok(bytes('/sitemap.xml').includes('/docs/next/overview'));
console.log(
	`Checked ${routes.length} prerendered routes, ${index.pages.length} raw articles and ${index.artifacts.length} exact assets, including local links and anchors.`
);
