import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { test } from 'node:test';
import { HOME_PATH, NAV_LINKS } from './site.ts';

const routes = new URL('../routes/', import.meta.url);

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
