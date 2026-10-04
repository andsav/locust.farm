import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { test } from 'node:test';
import { GUIDE_PATH } from './onboarding/guide.ts';
import { DOCS_PATH, HOME_PATH, NAV_LINKS } from './site.ts';

const routes = new URL('../routes/', import.meta.url);

const homepageSource = readFileSync(new URL('+page.svelte', routes), 'utf8');
const startSource = readFileSync(new URL('start/+page.svelte', routes), 'utf8');
const docsSource = readFileSync(new URL('docs/+page.svelte', routes), 'utf8');

const description = 'Unleash collective intelligence on your hardest problems';

test('the header links to the guide and docs after the home brand', () => {
	assert.equal(HOME_PATH, '/');
	assert.deepEqual(NAV_LINKS, [
		{ label: 'start', href: GUIDE_PATH },
		{ label: 'docs', href: DOCS_PATH }
	]);
});

test('every header link leads to a local route that exists', () => {
	for (const href of [HOME_PATH, ...NAV_LINKS.map((link) => link.href)]) {
		assert.ok(href.startsWith('/') && !href.startsWith('//'), `not a local path: ${href}`);
		assert.ok(!href.includes('#'), href);
		assert.ok(existsSync(new URL(`.${href.replace(/\/$/, '')}/+page.svelte`, routes)), href);
	}
});

test('the docs route is a clear placeholder', () => {
	assert.equal(DOCS_PATH, '/docs');
	assert.ok(docsSource.includes('<title>Docs — locust.farm</title>'));
	assert.ok(docsSource.includes('<h1>Documentation<span class="accent">.</span></h1>'));
	assert.ok(docsSource.includes('documentation will live'));
});

test('the homepage keeps its original heading, title and description', () => {
	assert.ok(
		homepageSource.includes('<h1>Distributed Agent Swarm<span class="accent">.</span></h1>'),
		'homepage H1 was changed'
	);
	assert.ok(
		homepageSource.includes('<title>locust.farm — Distributed Agent Swarm</title>'),
		'homepage title was changed'
	);
	assert.ok(homepageSource.includes(description), 'homepage description was changed');
});

test('rejected slogans stay off the site', () => {
	const rejected = [
		'Many agents, one hard task',
		'too hard for one agent',
		'best way to see the work'
	];
	for (const source of [homepageSource, startSource, docsSource]) {
		const lowered = source.toLowerCase();
		for (const phrase of rejected) {
			assert.ok(!lowered.includes(phrase.toLowerCase()), `rejected slogan present: ${phrase}`);
		}
	}
	assert.ok(
		!homepageSource.toLowerCase().includes('locust swarm'),
		'homepage must not reference the unimplemented locust swarm command'
	);
});
