import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { test } from 'node:test';
import {
	AGENT_RULE,
	AGENT_STEPS,
	ENTRY_PROMPT,
	GUIDE_URL,
	HARNESS_ROUTES,
	ROUTING_QUESTIONS,
	SETUP_ARTIFACT
} from './guide.ts';
import { articleFor, artifactFor } from '../docs/content.ts';
import { LLMS_PATH, llmsText, SUMMARY } from './llms.ts';

const routes = new URL('../../routes/', import.meta.url);
const text = llmsText();

test('the site serves llms.txt from a prerendered route', () => {
	const source = readFileSync(new URL(`.${LLMS_PATH}/+server.ts`, routes), 'utf8');
	assert.ok(source.includes('export const prerender = true'));
});

test('llms.txt opens with the name and the homepage description', () => {
	assert.ok(text.startsWith(`# Locust\n\n> ${SUMMARY}\n`));
	assert.ok(readFileSync(new URL('+page.svelte', routes), 'utf8').includes(SUMMARY));
});

test('llms.txt links only to pages the site serves', () => {
	const origin = new URL(GUIDE_URL).origin;
	const links = [...text.matchAll(/\]\((\S+?)\)/g)].map((match) => match[1]);
	assert.ok(links.includes(GUIDE_URL));
	for (const link of links) {
		const url = new URL(link);
		assert.equal(url.origin, origin, link);
		assert.ok(
			existsSync(new URL(`.${url.pathname.replace(/\/$/, '')}/+page.svelte`, routes)) ||
				url.pathname === '/docs/next/index.json' ||
				Boolean(articleFor(url.pathname.replace('/docs/next/', ''))) ||
				Boolean(artifactFor(url.pathname)),
			link
		);
	}
});

test('llms.txt carries the entry prompt and the guide’s instructions for agents', () => {
	assert.ok(text.includes(`\n> ${ENTRY_PROMPT}\n`));
	assert.ok(text.includes(AGENT_STEPS.identify));
	assert.ok(text.includes(AGENT_RULE));
	for (const { question } of ROUTING_QUESTIONS) assert.ok(text.includes(question));
	for (const route of HARNESS_ROUTES) {
		assert.ok(text.includes(`### ${route.name}\n`), route.name);
		assert.ok(text.includes(route.locust), route.name);
	}
});

test('llms.txt tells agents to stop while no setup is published', () => {
	if (SETUP_ARTIFACT) return;
	assert.ok(text.includes(`4. ${AGENT_STEPS.report}`));
	assert.ok(text.includes('nothing to install'));
	const links = text.match(/https?:\/\/[^\s)]+/g) ?? [];
	for (const link of links) assert.equal(new URL(link).hostname, 'locust.farm', link);
});
