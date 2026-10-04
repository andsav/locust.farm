// Holds the TypeScript checks to Locust: every case in
// docs/reference/conformance/organization.cases.json was run through
// `locust blueprint validate` by scripts/check_blueprints.py, and the results
// are in organization.vectors.json.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import vectors from '../../../../../../docs/reference/generated/organization.vectors.json' with { type: 'json' };
import { inspect } from './inspect.ts';

// Locust takes these messages from its JSON library; the page writes its own.
const OWN_MESSAGES = new Set(['invalid_json', 'duplicate_key', 'invalid_structure']);

interface VectorDiagnostic {
	code: string;
	severity: string;
	phase: string;
	path: string;
	message: string;
	correction: string;
	related_paths: string[];
}

for (const vector of vectors.cases) {
	test(`agrees with Locust: ${vector.id}`, () => {
		const expected = vector.result;
		const actual = inspect(vector.source);
		assert.equal(actual.valid, expected.valid, 'valid');
		const expectedDiagnostics = expected.diagnostics as VectorDiagnostic[];
		assert.deepEqual(
			actual.diagnostics.map((d) => [d.code, d.severity, d.phase, d.path]),
			expectedDiagnostics.map((d) => [d.code, d.severity, d.phase, d.path]),
			'codes and paths'
		);
		actual.diagnostics.forEach((d, index) => {
			const want = expectedDiagnostics[index];
			assert.equal(d.correction, want.correction, `correction of ${d.code}`);
			assert.deepEqual(d.related_paths, want.related_paths);
			if (!OWN_MESSAGES.has(d.code)) assert.equal(d.message, want.message, `message of ${d.code}`);
		});
		assert.deepEqual(actual.explanation, expected.explanation, 'explanation');
		assert.deepEqual(actual.normalized, expected.normalized, 'normalized');
	});
}

test('every diagnostic code Locust can report has a case', () => {
	const codes = new Set(
		vectors.cases.flatMap((vector) =>
			(vector.result.diagnostics as VectorDiagnostic[]).map((d) => d.code)
		)
	);
	for (const code of [
		'invalid_json',
		'duplicate_key',
		'unsupported_version',
		'invalid_structure',
		'invalid_name',
		'unknown_role',
		'invalid_participant',
		'selector_scope',
		'empty_selector',
		'impossible_completion',
		'invalid_threshold',
		'impossible_threshold',
		'empty_criteria',
		'unknown_task_type',
		'unknown_stage',
		'unavailable_evidence',
		'flow_cycle'
	]) {
		assert.ok(codes.has(code), code);
	}
});
