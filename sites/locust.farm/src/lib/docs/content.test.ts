import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import {
	artifactFor,
	articleFor,
	inventory,
	manifest,
	parseArticle,
	resolveLink,
	validateManifest
} from './content.ts';

test('manifest selects substantive articles and unknown routes select nothing', () => {
	validateManifest(manifest);
	for (const page of manifest.pages) {
		const article = articleFor(page.slug)!;
		assert.ok(article.text.length > 500);
		assert.ok(article.headings.length > 2);
	}
	for (const slug of ['unknown', '../overview', '../../site.json', 'overview.md', ''])
		assert.equal(articleFor(slug), undefined);
});
test('duplicate routes and traversal sources fail manifest validation', () => {
	assert.throws(
		() => validateManifest({ ...manifest, pages: [manifest.pages[0], manifest.pages[0]] }),
		/duplicate/
	);
	assert.throws(
		() =>
			validateManifest({
				...manifest,
				pages: [{ ...manifest.pages[0], source: 'docs/guide/../../secret.md' }]
			}),
		/Invalid/
	);
});
test('parser escapes HTML and code, renders tables and assigns unique anchors', () => {
	const parsed = parseArticle(
		'# A\n\n## Repeat\n\n## Repeat\n\n<script>alert(1)</script>\n\n```js\n<img src=x>\n```\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n',
		'docs/guide/overview.md'
	);
	assert.deepEqual(
		parsed.headings.map((h) => h.id),
		['a', 'repeat', 'repeat-2']
	);
	assert.ok(parsed.html.includes('<table>'));
	assert.ok(parsed.html.includes('&lt;script&gt;'));
	assert.ok(parsed.html.includes('&lt;img'));
	assert.ok(!parsed.html.includes('<script>'));
});
test('public link rewriting agrees across raw and HTML and rejects missing targets', () => {
	const parsed = parseArticle('[Concepts](concepts.md#goals)', 'docs/guide/overview.md');
	assert.equal(parsed.raw, '[Concepts](/docs/next/concepts#goals)');
	assert.ok(parsed.html.includes('href="/docs/next/concepts#goals"'));
	assert.throws(
		() => parseArticle('[Bad](concepts.md#missing)', 'docs/guide/overview.md'),
		/Missing anchor/
	);
	assert.throws(() => parseArticle('[Bad](missing.md)', 'docs/guide/overview.md'), /Missing/);
});
test('unsafe schemes and outside-repository paths are refused', () => {
	for (const href of [
		'javascript:alert(1)',
		'data:text/html,evil',
		'//evil.test',
		'../../../../etc/passwd'
	])
		assert.throws(() => resolveLink(href, 'docs/guide/overview.md'));
});
test('inventory hashes exactly served Markdown and shares status and version context', () => {
	const index = inventory();
	assert.match(index.sourceCommit, /^[0-9a-f]{40}$/);
	assert.equal(index.versions.formationSchema, 1);
	for (const page of index.pages)
		assert.equal(
			page.sha256,
			createHash('sha256').update(articleFor(page.slug)!.raw).digest('hex')
		);
});

test('generated references and exact example assets share the exported Rust contract', () => {
	const contract = JSON.parse(
		artifactFor('/docs/next/reference/organization.contract.json')!.bytes
	);
	const schema = JSON.parse(artifactFor('/docs/next/reference/organization.schema.json')!.bytes);
	assert.deepEqual(schema, contract.schema);
	const reference = articleFor('schema-reference')!;
	for (const operation of contract.operations)
		assert.ok(reference.text.includes(`locust formation ${operation.name}`));
	for (const example of contract.examples)
		assert.deepEqual(
			JSON.parse(artifactFor(`/docs/next/examples/${example.name}.json`)!.bytes),
			example.formation
		);
	for (const artifact of inventory().artifacts)
		assert.equal(
			artifact.sha256,
			createHash('sha256').update(artifactFor(artifact.url)!.bytes).digest('hex')
		);
	assert.equal(artifactFor('/docs/next/reference/../../site.json'), undefined);
	assert.throws(() => resolveLink('../README.md', 'docs/guide/overview.md'), /Unapproved/);
});

test('availability distinguishes the published CLI from unqualified first-contact setup', () => {
	const facts = JSON.parse(artifactFor('/docs/next/reference/availability.json')!.bytes);
	assert.equal(facts.publication.software, 'published-developer-preview');
	assert.equal(facts.publication.terminalInstallerUrl, 'https://locust.farm/downloads/install.sh');
	assert.equal(facts.publication.installerUrl, null);
	for (const client of Object.values(facts.clients) as { publicRoute: boolean }[])
		assert.equal(client.publicRoute, false);
	for (const record of facts.qualification)
		for (const source of record.evidence)
			assert.ok(readFileSync(new URL(`../../../../../${source}`, import.meta.url), 'utf8').length);
});

test('all frozen manual subjects resolve to substantive canonical content', () => {
	const plan = readFileSync(new URL('../../../../../docs/manual.md', import.meta.url), 'utf8');
	const inventorySection = plan.slice(
		plan.indexOf('## 5. Complete manual inventory'),
		plan.indexOf('## 6. Build and site implementation')
	);
	const required = inventorySection
		.split('\n')
		.filter((line) => line.startsWith('|'))
		.flatMap((line) =>
			[...(line.split('|')[2] ?? '').matchAll(/`([^`]+)`/g)].map((match) => match[1])
		);
	assert.ok(required.length > 60);
	for (const slug of required) {
		const article = articleFor(slug);
		assert.ok(article && article.text.length > 500, slug);
		if (article.section)
			assert.ok(
				article.headings.some((heading) => heading.id === article.section),
				slug
			);
	}
});

test('documentation source markers match current Rust API and protocol identifiers', () => {
	const protocol = readFileSync(
		new URL('../../../../../crates/locust-proto/src/lib.rs', import.meta.url),
		'utf8'
	);
	assert.equal(
		manifest.versions.api,
		Number(protocol.match(/pub const API_VERSION: u16 = (\d+)/)![1])
	);
	assert.equal(
		manifest.versions.protocol,
		Number(protocol.match(/pub const PROTOCOL_VERSION: u8 = (\d+)/)![1])
	);
});

test('runtime reference renders the executable operation and CLI contract', () => {
	const runtime = JSON.parse(artifactFor('/docs/next/reference/runtime.contract.json')!.bytes);
	const reference = articleFor('runtime-reference')!;
	assert.equal(runtime.api_version, manifest.versions.api);
	assert.equal(runtime.protocol_version, manifest.versions.protocol);
	for (const operation of runtime.operations) {
		assert.ok(reference.text.includes(operation.name), operation.name);
		if (operation.mcp_tool) assert.ok(reference.text.includes(operation.mcp_tool));
	}
	assert.ok(reference.text.includes('locust contract'));
	assert.ok(reference.text.includes('contribution_published'));
	assert.ok(reference.text.includes('effect_materialized'));
	assert.ok(runtime.response_schema.$defs.Draft);
	assert.equal(articleFor('reference/cli')!.section, 'generated-cli');
});
