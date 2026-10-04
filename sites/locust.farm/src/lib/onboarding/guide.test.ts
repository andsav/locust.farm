import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { test } from 'node:test';
import {
	ENTRY_PROMPT,
	GUIDE_PATH,
	GUIDE_URL,
	HARNESS_ROUTES,
	ROUTING_QUESTIONS,
	SETUP_ARTIFACT
} from './guide.ts';

const routes = new URL('../../routes/', import.meta.url);
const contract = new URL('../../../../../docs/first-contact.md', import.meta.url);

test('the entry prompt points at the absolute guide address, which the site serves', () => {
	const url = new URL(GUIDE_URL);
	assert.equal(url.protocol, 'https:');
	assert.equal(url.pathname, GUIDE_PATH);
	assert.ok(ENTRY_PROMPT.startsWith(`Read ${GUIDE_URL} `));
	assert.ok(existsSync(new URL(`.${GUIDE_PATH}/+page.svelte`, routes)));
});

test('the entry prompt is the one in the first-contact contract', () => {
	assert.ok(readFileSync(contract, 'utf8').includes(`\n> ${ENTRY_PROMPT}\n`));
});

test('the entry prompt asks for approval and carries no command, download or secret', () => {
	assert.match(ENTRY_PROMPT, /Do not install or change anything until I approve\./);
	const links = ENTRY_PROMPT.match(/https?:\/\/\S+/g);
	assert.deepEqual(links, [GUIDE_URL]);
	for (const pattern of [
		/\$\s/,
		/\|/,
		/`/,
		/\bcurl\b/,
		/\bnpm\b/,
		/\bsh\b/,
		/token|ticket|secret/i
	]) {
		assert.doesNotMatch(ENTRY_PROMPT, pattern);
	}
});

test('the four baseline harnesses and any other harness each have a route', () => {
	assert.deepEqual(
		HARNESS_ROUTES.map((route) => route.name),
		['Claude Code', 'Codex', 'pi', 'Droid', 'Any other harness']
	);
	for (const route of HARNESS_ROUTES) {
		assert.ok(route.upstream && route.locust && route.prerequisite, route.name);
	}
	assert.deepEqual(
		ROUTING_QUESTIONS.map((question) => question.name),
		['Transport', 'Instructions', 'Approvals']
	);
});

test('no route claims support while no setup is published', () => {
	if (SETUP_ARTIFACT) return;
	for (const route of HARNESS_ROUTES) {
		assert.doesNotMatch(route.locust, /\b(supported|ready|works)\b/i, route.name);
		assert.match(route.locust, /\bnot\b|\bno\b/i, route.name);
	}
});
