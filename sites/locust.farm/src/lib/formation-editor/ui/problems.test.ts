import assert from 'node:assert/strict';
import { test } from 'node:test';
import { inspect } from '../contract/inspect.ts';
import { fromDiagnostics } from './problems.ts';
import { newDocument } from '../model/edit.ts';

test('a stage creator diagnostic explains who can do the work', () => {
	const document = newDocument();
	document.formation.flow.draft = {
		task_type: null,
		requires: [],
		recipients: { kind: 'members' }
	};
	document.formation.decisions.completion = {
		kind: 'declaration',
		by: { kind: 'task_creator' }
	};
	const inspection = inspect(JSON.stringify(document.formation));
	assert.equal(inspection.valid, false);
	const problem = fromDiagnostics(inspection.diagnostics, document.formation).find((problem) =>
		problem.technical?.includes('task_creator at')
	);
	assert.ok(problem);
	assert.match(problem.text, /host's computer/);
	assert.match(problem.text, /Name members, a role or a specific member/);
	assert.doesNotMatch(problem.text, /author of the result/);
});
