import assert from 'node:assert/strict';
import { test } from 'node:test';
import { age, clockTime } from './time.ts';

test('unknown observation times remain unknown and future observations do not imply elapsed time', () => {
	assert.equal(age(null, 1000), 'unknown');
	assert.equal(clockTime(null), 'unknown');
	assert.equal(age(2000, 1000), 'ahead of service clock');
	assert.equal(age(1000, 122000), '2 min ago');
	assert.equal(age(NaN, 1000), 'unknown');
	assert.equal(clockTime(Number.MAX_VALUE), 'unknown');
});
