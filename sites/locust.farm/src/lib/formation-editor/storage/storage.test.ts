import assert from 'node:assert/strict';
import { test } from 'node:test';
import { newDocument } from '../model/edit.ts';
import { openText } from '../prompt/open.ts';
import { blocks } from '../prompt/prompt.ts';
import { decodeLink, encodeLink, MAX_FRAGMENT } from './link.ts';
import { listSaved, loadSaved, removeSaved, save, type KeyValueStore } from './saved.ts';

function memoryStore(): KeyValueStore & { map: Map<string, string> } {
	const map = new Map<string, string>();
	return {
		map,
		getItem: (key) => map.get(key) ?? null,
		setItem: (key, value) => void map.set(key, value),
		removeItem: (key) => void map.delete(key)
	};
}

test('saved formations are separate records listed newest first', () => {
	const store = memoryStore();
	const record = (id: string, saved: string) => ({
		id,
		name: id,
		origin: 'new' as const,
		saved,
		data: '{}'
	});
	assert.ok(save(store, record('a', '2026-10-04T10:00:00Z')));
	assert.ok(save(store, record('b', '2026-10-04T11:00:00Z')));
	assert.ok(save(store, record('a', '2026-10-04T12:00:00Z')));
	assert.deepEqual(
		listSaved(store).map((item) => item.id),
		['a', 'b']
	);
	assert.equal(loadSaved(store, 'b')?.name, 'b');
	assert.ok(removeSaved(store, 'a'));
	assert.deepEqual(
		listSaved(store).map((item) => item.id),
		['b']
	);
	assert.equal(loadSaved(store, 'a'), null);
});

test('a full or blocked store reports failure instead of throwing', () => {
	const store: KeyValueStore = {
		getItem: () => null,
		setItem: () => {
			throw new Error('QuotaExceededError');
		},
		removeItem: () => {}
	};
	assert.equal(save(store, { id: 'x', name: '', origin: 'new', saved: '', data: '' }), false);
});

test('a share link round-trips a formation and its layout', async () => {
	const document = { ...newDocument('pipeline'), name: 'Sync research' };
	const data = await blocks(document);
	const fragment = await encodeLink(data.text);
	assert.match(fragment, /^b1\.[A-Za-z0-9_-]+$/);
	const decoded = await decodeLink(`#${fragment}`);
	assert.ok(decoded && decoded.ok);
	if (decoded && decoded.ok) {
		const opened = openText(decoded.text);
		assert.ok(opened.ok);
		if (opened.ok) {
			assert.deepEqual(opened.document.formation, document.formation);
			assert.equal(opened.document.name, 'Sync research');
		}
	}
});

test('damaged, oversized and unrelated fragments are handled', async () => {
	assert.equal(await decodeLink('#section-2'), null);
	const damaged = await decodeLink('#b1.!!!!');
	assert.ok(damaged && !damaged.ok);
	const huge = await decodeLink(`#b1.${'A'.repeat(MAX_FRAGMENT)}`);
	assert.ok(huge && !huge.ok);
	const bomb = await encodeLink('x'.repeat(2 * 1024 * 1024));
	const expanded = await decodeLink(bomb);
	assert.ok(expanded && !expanded.ok);
});
