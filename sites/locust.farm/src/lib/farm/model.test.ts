import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { associations, farmMode, harnessName, stageColumns } from './model.ts';
import type { FarmSnapshot, Harness } from './types.ts';
const fixture: FarmSnapshot = JSON.parse(
	readFileSync(
		new URL('../../../../../crates/locust-proto/fixtures/farm-snapshot.json', import.meta.url),
		'utf8'
	)
);

test('parallel stages keep stable topology and unstaged work stays independent', () => {
	assert.deepEqual(stageColumns(fixture), [[1], [2, 3], [4], [5]]);
	assert.deepEqual(
		associations(fixture, null).map(({ agent }) => agent.id),
		[2]
	);
});
test('all current attempt associations appear, while historical rounds do not place agents', () => {
	const snapshot = structuredClone(fixture);
	snapshot.attempts.push({ id: 100, agent: 1, task: 3, round: 2, state: 'completed' });
	const frontend = associations(snapshot, 2);
	assert.deepEqual(
		frontend.map(({ agent }) => agent.id),
		[2, 4]
	);
	assert.equal(frontend[0].attempts.length, 2);
	assert.equal(frontend[1].attempts[0].task.id, 4);
});
test('service availability overrides closure and closure overrides receipt age', () => {
	const view = {
		farm_id: fixture.farm_id,
		stream_version: 1,
		service_time_ms: 1000,
		received_at_ms: 1000,
		snapshot: fixture
	};
	assert.equal(farmMode(view, 1001), 'receiving');
	assert.equal(farmMode(view, 121001), 'quiet');
	assert.equal(
		farmMode({ ...view, snapshot: { ...fixture, goal_state: 'ended' } }, 121001),
		'ended'
	);
	assert.equal(farmMode({ ...view, status: 'unavailable' }, 1001), 'unavailable');
});

test('canonical Kimi Code reports display their approved harness label', () => {
	const reported: Harness = 'kimi_code';
	assert.equal(harnessName(reported), 'Kimi Code');
	assert.equal(harnessName('Kimi Code'), 'Harness unknown');
});
