import assert from 'node:assert/strict';
import { test } from 'node:test';
import { copyText } from './clipboard.ts';

test('copies exactly the given text and reports success', async () => {
	const seen: string[] = [];
	const clipboard = {
		async writeText(text: string) {
			seen.push(text);
		}
	};
	const input = 'Read https://example.test/guide, then ask me: "ready?" Do nothing else.';
	const result = await copyText(input, clipboard);
	assert.equal(result, 'copied');
	assert.equal(seen.length, 1);
	assert.equal(seen[0], input);
});

test('reports failure when the write is rejected', async () => {
	const clipboard = {
		writeText() {
			return Promise.reject(new Error('denied'));
		}
	};
	const result = await copyText('anything', clipboard);
	assert.equal(result, 'failed');
});

test('reports failure when writeText throws synchronously', async () => {
	const clipboard = {
		writeText() {
			throw new Error('boom');
		}
	};
	const result = await copyText('anything', clipboard);
	assert.equal(result, 'failed');
});

test('reports failure when no clipboard is available', async () => {
	const result = await copyText('anything', undefined);
	assert.equal(result, 'failed');
});

test('success is only reported after the write settles', async () => {
	let resolve: () => void = () => {};
	const pending = new Promise<void>((res) => {
		resolve = res;
	});
	const clipboard = {
		writeText() {
			return pending;
		}
	};

	let settled = false;
	const promise = copyText('anything', clipboard).then((r) => {
		settled = true;
		return r;
	});

	// Yield to the microtask queue a few times: the pending write must hold the result back.
	for (let i = 0; i < 3; i++) {
		await Promise.resolve();
	}
	assert.equal(settled, false, 'copyText must not settle before the write settles');

	resolve();
	const result = await promise;
	assert.equal(settled, true);
	assert.equal(result, 'copied');
});
