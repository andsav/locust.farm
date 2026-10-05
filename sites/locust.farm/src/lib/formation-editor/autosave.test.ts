import assert from 'node:assert/strict';
import { test } from 'node:test';
import { newDocument, setName } from './model/edit.ts';
import { Autosaver, type AutosaveTimers, type SaveStatus, type TimerHandle } from './autosave.ts';
import type { EditorDocument } from './model/document.ts';
import type { KeyValueStore, SavedRecord } from './storage/saved.ts';

// A non-null store sentinel: persist is injected, so the store value only needs
// to be truthy so the autosaver hashes instead of reporting "no store".
const STORE: KeyValueStore = {
	getItem: () => null,
	setItem: () => {},
	removeItem: () => {}
};

function manualTimers() {
	let callback: (() => void) | null = null;
	let handle = 0;
	return {
		timers: {
			set: (cb: () => void) => {
				callback = cb;
				return ++handle as unknown as TimerHandle;
			},
			clear: () => {
				callback = null;
			}
		} satisfies AutosaveTimers,
		fire: () => {
			const cb = callback;
			callback = null;
			cb?.();
		},
		isPending: () => callback !== null
	};
}

function deferredHash() {
	const queue: Array<(text: string) => void> = [];
	return {
		hash: () => new Promise<string>((resolve) => queue.push(resolve)),
		resolveNext: (text: string) => {
			const r = queue.shift();
			r?.(text);
		},
		resolveLast: (text: string) => {
			const r = queue.pop();
			r?.(text);
		},
		pending: () => queue.length
	};
}

interface Harness {
	autosaver: Autosaver;
	timers: ReturnType<typeof manualTimers>;
	hash: ReturnType<typeof deferredHash>;
	persisted: SavedRecord[];
	statuses: SaveStatus[];
	setCurrent: (id: string) => void;
}

function harness(): Harness {
	const timers = manualTimers();
	const hash = deferredHash();
	const persisted: SavedRecord[] = [];
	const statuses: SaveStatus[] = [];
	let current = '';
	const autosaver = new Autosaver({
		debounceMs: 400,
		timers: timers.timers,
		hash: hash.hash,
		persist: (_store, record) => {
			persisted.push(record);
			return true;
		},
		currentRecordId: () => current,
		onStatus: (status) => statuses.push(status)
	});
	return {
		autosaver,
		timers,
		hash,
		persisted,
		statuses,
		setCurrent: (id) => {
			current = id;
		}
	};
}

function snapshot(
	document: EditorDocument,
	recordId: string
): {
	document: EditorDocument;
	recordId: string;
	origin: 'new';
} {
	return { document, recordId, origin: 'new' };
}

// Let all pending promise continuations settle before assertions.
const settle = () => new Promise<void>((r) => setTimeout(r, 0));

test('switching before the debounce flushes the prior edit under its own id', async () => {
	const h = harness();
	h.setCurrent('A');
	const docA = setName(newDocument(), 'Alpha');
	h.autosaver.schedule(STORE, snapshot(docA, 'A'));
	assert.ok(h.timers.isPending());
	assert.equal(h.hash.pending(), 0);
	// Switch: flush (fire-and-forget) then change record identity immediately.
	const flushing = h.autosaver.flush();
	h.setCurrent('B');
	assert.equal(h.hash.pending(), 1);
	assert.ok(!h.timers.isPending());
	h.hash.resolveNext('alpha-data');
	await flushing;
	assert.deepEqual(
		h.persisted.map((r) => r.id),
		['A']
	);
	assert.equal(h.persisted[0].data, 'alpha-data');
	assert.equal(h.persisted[0].name, 'Alpha');
	// The viewer moved to B, so A's completion does not change the status.
	assert.equal(h.statuses.length, 0);
});

test('switching during hashing writes the prior data under the prior id, not the new one', async () => {
	const h = harness();
	h.setCurrent('A');
	const docA = setName(newDocument(), 'Alpha');
	h.autosaver.schedule(STORE, snapshot(docA, 'A'));
	h.timers.fire(); // debounce elapses; hashing starts
	assert.equal(h.hash.pending(), 1);
	// Switch during hashing: schedule B's save while A still hashes.
	h.setCurrent('B');
	const docB = setName(newDocument(), 'Beta');
	h.autosaver.schedule(STORE, snapshot(docB, 'B'));
	// A's hash resolves; it must write under A, not B.
	h.hash.resolveNext('alpha-data');
	await settle();
	assert.deepEqual(
		h.persisted.map((r) => r.id),
		['A']
	);
	assert.equal(h.persisted[0].data, 'alpha-data');
	assert.equal(h.statuses.length, 0);
	// B's debounce fires and saves under B.
	h.timers.fire();
	h.hash.resolveNext('beta-data');
	await settle();
	assert.deepEqual(
		h.persisted.map((r) => r.id),
		['A', 'B']
	);
	assert.equal(h.persisted[1].data, 'beta-data');
	assert.equal(h.statuses.length, 1);
	assert.ok(h.statuses[0].ok);
});

test('rapid same-record edits keep the newer data and the older completion retires', async () => {
	const h = harness();
	h.setCurrent('A');
	const docA1 = setName(newDocument(), 'Alpha one');
	const docA2 = setName(newDocument(), 'Alpha two');
	h.autosaver.schedule(STORE, snapshot(docA1, 'A'));
	h.timers.fire(); // edit 1 starts hashing
	assert.equal(h.hash.pending(), 1);
	// A second edit to the same record schedules a newer save while edit 1 hashes.
	h.autosaver.schedule(STORE, snapshot(docA2, 'A'));
	assert.ok(h.timers.isPending());
	// Edit 2's debounce fires and starts hashing too.
	h.timers.fire();
	assert.equal(h.hash.pending(), 2);
	// Edit 2 completes first; its data is the newer one for record A.
	h.hash.resolveLast('alpha-two');
	await settle();
	assert.deepEqual(
		h.persisted.map((r) => r.data),
		['alpha-two']
	);
	assert.equal(h.statuses.length, 1);
	assert.ok(h.statuses[0].ok);
	// Edit 1 completes later; it must not overwrite edit 2's data or status.
	h.hash.resolveNext('alpha-one');
	await settle();
	assert.deepEqual(
		h.persisted.map((r) => r.data),
		['alpha-two']
	);
	assert.equal(h.statuses.length, 1);
});

test('a newer pending same-record save retires an older completion before it writes', async () => {
	const h = harness();
	h.setCurrent('A');
	const docA1 = setName(newDocument(), 'Alpha one');
	const docA2 = setName(newDocument(), 'Alpha two');
	h.autosaver.schedule(STORE, snapshot(docA1, 'A'));
	h.timers.fire(); // edit 1 hashing
	// Edit 2 is scheduled but its debounce has not fired when edit 1 finishes.
	h.autosaver.schedule(STORE, snapshot(docA2, 'A'));
	assert.ok(h.timers.isPending());
	assert.equal(h.hash.pending(), 1);
	h.hash.resolveNext('alpha-one');
	await settle();
	// Edit 1 retires because a newer same-record save is pending; nothing is written yet.
	assert.deepEqual(
		h.persisted.map((r) => r.data),
		[]
	);
	assert.equal(h.statuses.length, 0);
	// Edit 2 fires and writes the newer data.
	h.timers.fire();
	h.hash.resolveNext('alpha-two');
	await settle();
	assert.deepEqual(
		h.persisted.map((r) => r.data),
		['alpha-two']
	);
	assert.equal(h.statuses.length, 1);
	assert.ok(h.statuses[0].ok);
});

test('teardown clears the debounce timer and flushes the last edit', async () => {
	const h = harness();
	h.setCurrent('A');
	const docA = setName(newDocument(), 'Alpha');
	h.autosaver.schedule(STORE, snapshot(docA, 'A'));
	assert.ok(h.timers.isPending());
	h.autosaver.teardown();
	assert.ok(!h.timers.isPending());
	h.timers.fire();
	assert.equal(h.hash.pending(), 1);
	h.hash.resolveNext('alpha-data');
	await settle();
	assert.deepEqual(
		h.persisted.map((r) => r.id),
		['A']
	);
	assert.equal(h.statuses.length, 0);
	// Further schedules are refused after teardown.
	h.autosaver.schedule(STORE, snapshot(docA, 'A'));
	assert.ok(!h.timers.isPending());
});

test('an in-flight save at teardown still writes under its captured id', async () => {
	const h = harness();
	h.setCurrent('A');
	const docA = setName(newDocument(), 'Alpha');
	h.autosaver.schedule(STORE, snapshot(docA, 'A'));
	h.timers.fire(); // hashing starts
	assert.equal(h.hash.pending(), 1);
	h.autosaver.teardown();
	// The record identity has already been captured; the save finishes safely.
	h.setCurrent('B');
	h.hash.resolveNext('alpha-data');
	await settle();
	assert.deepEqual(
		h.persisted.map((r) => r.id),
		['A']
	);
	assert.equal(h.persisted[0].data, 'alpha-data');
	// Teardown suppresses status updates.
	assert.equal(h.statuses.length, 0);
});

test('a refused store reports failure and keeps the meaningful status', async () => {
	const timers = manualTimers();
	const hash = deferredHash();
	const statuses: SaveStatus[] = [];
	const current = 'A';
	const autosaver = new Autosaver({
		debounceMs: 400,
		timers: timers.timers,
		hash: hash.hash,
		persist: () => false,
		currentRecordId: () => current,
		onStatus: (status) => statuses.push(status)
	});
	const docA = setName(newDocument(), 'Alpha');
	autosaver.schedule(STORE, snapshot(docA, 'A'));
	timers.fire();
	hash.resolveNext('alpha-data');
	await settle();
	assert.equal(statuses.length, 1);
	assert.equal(statuses[0].ok, false);
	assert.match(statuses[0].text, /this browser refused to store it/);
});

test('a missing store reports the no-store status for the current record', async () => {
	const timers = manualTimers();
	const statuses: SaveStatus[] = [];
	let current = 'A';
	const autosaver = new Autosaver({
		debounceMs: 400,
		timers: timers.timers,
		currentRecordId: () => current,
		onStatus: (status) => statuses.push(status)
	});
	const docA = setName(newDocument(), 'Alpha');
	autosaver.schedule(null, snapshot(docA, 'A'));
	timers.fire();
	await settle();
	assert.equal(statuses.length, 1);
	assert.equal(statuses[0].ok, false);
	assert.match(statuses[0].text, /isn't keeping data for this page/);
	// After switching away, the no-store status is not re-reported for the old record.
	current = 'B';
	statuses.length = 0;
	autosaver.schedule(null, snapshot(docA, 'A'));
	timers.fire();
	await settle();
	assert.equal(statuses.length, 0);
});

test('another record completing does not allow an older version to overwrite its record', async () => {
	const h = harness();
	h.autosaver.schedule(STORE, snapshot(setName(newDocument(), 'A old'), 'A'));
	h.timers.fire();
	h.autosaver.schedule(STORE, snapshot(setName(newDocument(), 'A new'), 'A'));
	h.timers.fire();
	h.hash.resolveLast('A new');
	await settle();
	h.autosaver.schedule(STORE, snapshot(setName(newDocument(), 'B'), 'B'));
	h.timers.fire();
	h.hash.resolveLast('B');
	await settle();
	h.hash.resolveNext('A old');
	await settle();
	assert.deepEqual(
		h.persisted.map(({ id, data }) => ({ id, data })),
		[
			{ id: 'A', data: 'A new' },
			{ id: 'B', data: 'B' }
		]
	);
});

test('flushing after the timer fires does not start duplicate hashing', async () => {
	const h = harness();
	h.autosaver.schedule(STORE, snapshot(newDocument(), 'A'));
	h.timers.fire();
	await h.autosaver.flush();
	assert.equal(h.hash.pending(), 1);
	h.hash.resolveNext('A');
	await settle();
	assert.equal(h.persisted.length, 1);
});

test('the saved snapshot is independent of later mutation of its input', async () => {
	const h = harness();
	const document = setName(newDocument(), 'Original');
	const input = snapshot(document, 'A');
	h.autosaver.schedule(STORE, input);
	document.name = 'Changed';
	input.recordId = 'B';
	h.timers.fire();
	h.hash.resolveNext('Original data');
	await settle();
	assert.equal(h.persisted[0].id, 'A');
	assert.equal(h.persisted[0].name, 'Original');
});

test('discarding a deleted record cancels both pending and in-flight saves', async () => {
	const h = harness();
	h.autosaver.schedule(STORE, snapshot(newDocument(), 'A'));
	h.timers.fire();
	h.autosaver.schedule(STORE, snapshot(newDocument(), 'A'));
	h.autosaver.discard('A');
	assert.equal(h.timers.isPending(), false);
	h.hash.resolveNext('deleted A');
	await settle();
	assert.deepEqual(h.persisted, []);
});

test('hashing failures are reported and do not prevent a later save', async () => {
	const timers = manualTimers();
	const statuses: SaveStatus[] = [];
	const persisted: SavedRecord[] = [];
	let fail = true;
	const autosaver = new Autosaver({
		debounceMs: 400,
		timers: timers.timers,
		hash: async () => {
			if (fail) throw new Error('failed');
			return 'saved';
		},
		persist: (_store, record) => {
			persisted.push(record);
			return true;
		},
		currentRecordId: () => 'A',
		onStatus: (status) => statuses.push(status)
	});
	autosaver.schedule(STORE, snapshot(newDocument(), 'A'));
	await autosaver.flush();
	assert.equal(statuses[0].ok, false);
	assert.equal(persisted.length, 0);
	fail = false;
	autosaver.schedule(STORE, snapshot(newDocument(), 'A'));
	await autosaver.flush();
	assert.equal(statuses[1].ok, true);
	assert.equal(persisted[0].data, 'saved');
});
