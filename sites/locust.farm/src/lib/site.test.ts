import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { test } from 'node:test';
import { HOME_PATH, NAV_LINKS } from './site.ts';

const routes = new URL('../routes/', import.meta.url);

const homepageSource = readFileSync(new URL('+page.svelte', routes), 'utf8');
const startSource = readFileSync(new URL('start/+page.svelte', routes), 'utf8');

const description =
	'Distributed multi-agent orchestration. Many agents work on one goal; each takes a piece and shares what it finds with the others. Open source.';

test('every header link leads to a real destination', () => {
	for (const href of [HOME_PATH, ...NAV_LINKS.map((link) => link.href)]) {
		assert.ok(!href.includes('#'), href);
		if (href.startsWith('/')) {
			assert.ok(existsSync(new URL(`.${href.replace(/\/$/, '')}/+page.svelte`, routes)), href);
		} else {
			assert.equal(new URL(href).protocol, 'https:', href);
		}
	}
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
	for (const source of [homepageSource, startSource]) {
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
