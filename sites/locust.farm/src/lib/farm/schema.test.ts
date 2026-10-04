import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { generateFarmTypes, targetPath } from '../../../scripts/generate-farm-types.mjs';

test('public snapshot types exactly match the exported Rust schema', async () => {
	assert.equal(await readFile(targetPath, 'utf8'), await generateFarmTypes());
});
