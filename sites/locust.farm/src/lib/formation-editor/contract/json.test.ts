import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseJson } from './json.ts';
import { compareCodePoints, escapePointer, rustDebug } from './text.ts';

test('keeps number text exactly', () => {
	const result = parseJson('{"a": 1.50, "b": -0, "c": 18446744073709551617}');
	assert.ok(result.ok);
	assert.deepEqual(result.value, {
		type: 'object',
		entries: [
			['a', { type: 'number', text: '1.50' }],
			['b', { type: 'number', text: '-0' }],
			['c', { type: 'number', text: '18446744073709551617' }]
		]
	});
});

test('refuses duplicate keys at the second key', () => {
	const result = parseJson('{"x": {"a~b": 1, "a~b": 2}}');
	assert.equal(result.ok, false);
	if (!result.ok) {
		assert.equal(result.error.code, 'duplicate_key');
		assert.equal(result.error.path, '/x/a~0b');
	}
});

test('limits nesting like serde_json', () => {
	assert.ok(parseJson('['.repeat(128) + ']'.repeat(128)).ok);
	const deep = parseJson('['.repeat(129) + ']'.repeat(129));
	assert.equal(deep.ok, false);
});

test('refuses lone surrogates, raw control characters and trailing text', () => {
	for (const source of ['"\\ud800"', '"\\udc00"', '"a\u0001"', '{} {}', '01', '1.', '[1,]', '']) {
		assert.equal(parseJson(source).ok, false, source);
	}
	assert.ok(parseJson('"\\ud83d\\ude00"').ok);
});

test('orders strings by code point, as Rust does', () => {
	const sorted = ['\u{1F600}', '\u{ff5e}', 'b', 'a'].sort(compareCodePoints);
	assert.deepEqual(sorted, ['a', 'b', '\u{ff5e}', '\u{1F600}']);
});

test('escapes pointers and debug-quotes names', () => {
	assert.equal(escapePointer('a/b~c'), 'a~1b~0c');
	assert.equal(rustDebug('say "hi" \\ r\u{e9}\n'), '"say \\"hi\\" \\\\ r\u{e9}\\n"');
	assert.equal(rustDebug('a\u0007'), '"a\\u{7}"');
});
