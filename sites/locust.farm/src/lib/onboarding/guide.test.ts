import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { test } from 'node:test';
import {
	AGENT_STEPS,
	ENTRY_PROMPT,
	GUIDE_PATH,
	GUIDE_URL,
	HARNESS_ROUTES,
	INSTALL_GUIDE_URL,
	INSTALLER_URL,
	PORTABLE_STEPS,
	ROUTING_QUESTIONS
} from './guide.ts';

const routes = new URL('../../routes/', import.meta.url);
const contract = new URL('../../../../../docs/first-contact.md', import.meta.url);

test('the guide exists but the prompt works without authenticated page access', () => {
	assert.equal(new URL(GUIDE_URL).pathname, GUIDE_PATH);
	assert.ok(existsSync(new URL(`.${GUIDE_PATH}/+page.svelte`, routes)));
	assert.ok(ENTRY_PROMPT.includes(INSTALL_GUIDE_URL));
	assert.ok(ENTRY_PROMPT.includes(INSTALLER_URL));
	assert.ok(ENTRY_PROMPT.includes('you do not need access to /start'));
	assert.doesNotMatch(ENTRY_PROMPT, /Read https:\/\/locust.farm\/start/);
});

test('the displayed and copied prompt is the first-contact contract', () => {
	assert.ok(readFileSync(contract, 'utf8').includes(`\n> ${ENTRY_PROMPT}\n`));
});

test('setup authorization covers updates and preserves identity without extra approval rounds', () => {
	assert.match(ENTRY_PROMPT, /Install or update/);
	assert.match(ENTRY_PROMPT, /update its existing software prefix/);
	assert.match(ENTRY_PROMPT, /preserve its daemon data, identity, credentials and sessions/);
	assert.match(ENTRY_PROMPT, /this request authorizes/);
	assert.match(ENTRY_PROMPT, /--yes/);
	assert.doesNotMatch(ENTRY_PROMPT, /until I approve|wait for.*approv/i);
	assert.match(AGENT_STEPS.update, /does not replace a running daemon/);
	assert.match(AGENT_STEPS.update, /Preserve incompatible existing state/);
	assert.match(ENTRY_PROMPT, /Do not create or join goals, grant work permissions or share files/);
});

test('the four named agents and other shell-capable agents have actionable routes', () => {
	assert.deepEqual(
		HARNESS_ROUTES.map((route) => route.name),
		['Claude Code', 'Codex', 'pi', 'Droid', 'Any other harness']
	);
	for (const route of HARNESS_ROUTES) {
		assert.match(route.locust, /--client (claude|codex|pi|droid|shell)/, route.name);
		assert.ok(route.upstream && route.prerequisite, route.name);
	}
	assert.deepEqual(
		ROUTING_QUESTIONS.map((question) => question.name),
		['Execution host', 'Transport', 'Instructions']
	);
	assert.ok(PORTABLE_STEPS.join('\n').includes('service plan'));
	assert.ok(PORTABLE_STEPS.join('\n').includes('agent enroll'));
	assert.ok(
		PORTABLE_STEPS.join('\n').includes('--credential CREDENTIAL_PATH --session SESSION_PATH status')
	);
});

test('an older public binary can finish setup without source-only client flags', () => {
	assert.match(ENTRY_PROMPT, /Inspect up --help/);
	assert.match(ENTRY_PROMPT, /Otherwise finish daemon setup with service plan\/apply\/start/);
	assert.match(AGENT_STEPS.route, /commands supported by those bytes/);
	const instructions = Object.values(AGENT_STEPS).join('\n');
	assert.doesNotMatch(instructions, /then stop|Do not install, download or run/);
	assert.match(instructions, /not a command to stop/);
});
