// The same pipeline as `locust blueprint validate`
// (crates/locust-core/src/organization.rs `inspect`), without the hashes:
// strict JSON, the schema version, structure, rules, then normalization and the
// explanation for a valid blueprint.

import { decodeBlueprint } from './decode.ts';
import { explain } from './explain.ts';
import { field, isNumber, isObject, parseJson } from './json.ts';
import { normalize } from './normalize.ts';
import { validate } from './rules.ts';
import { SCHEMA_VERSION, type Blueprint, type Diagnostic, type Inspection } from './types.ts';

const LOAD_CORRECTION =
	'Supply one complete JSON object with unique field names; parser resource-limit errors are reported without truncating the document.';
const VERSION_CORRECTION =
	'Use the contract exported by this build. Unsupported documents are not converted.';
const STRUCTURE_CORRECTION =
	'Use blueprint schema or a bundled example for supported fields and values.';

const U64_MAX = 18446744073709551615n;

function failed(diagnostic: Diagnostic): Inspection {
	return { valid: false, diagnostics: [diagnostic], normalized: null, explanation: null };
}

function diagnostic(
	code: string,
	phase: Diagnostic['phase'],
	path: string,
	message: string,
	correction: string
): Diagnostic {
	return { code, severity: 'error', phase, path, message, correction, related_paths: [] };
}

/** Inspects exact source text. */
export function inspect(source: string): Inspection {
	const parsed = parseJson(source);
	if (!parsed.ok) {
		const { code, path, message } = parsed.error;
		return failed(diagnostic(code, 'load', path, message, LOAD_CORRECTION));
	}
	const root = parsed.value;
	if (isObject(root)) {
		const version = field(root, 'schema_version');
		if (isNumber(version) && /^[0-9]+$/.test(version.text)) {
			const number = BigInt(version.text);
			if (number <= U64_MAX && number !== BigInt(SCHEMA_VERSION)) {
				return failed(
					diagnostic(
						'unsupported_version',
						'load',
						'/schema_version',
						`schema version ${number} is unsupported; this build accepts ${SCHEMA_VERSION}`,
						VERSION_CORRECTION
					)
				);
			}
		}
	}
	const decoded = decodeBlueprint(root);
	if (!decoded.ok) {
		return failed(
			diagnostic(
				'invalid_structure',
				'definition',
				decoded.error.path,
				decoded.error.message,
				STRUCTURE_CORRECTION
			)
		);
	}
	return inspectBlueprint(decoded.blueprint);
}

/** Inspects a blueprint the editor already holds as a value. */
export function inspectBlueprint(blueprint: Blueprint): Inspection {
	const diagnostics = validate(blueprint);
	if (diagnostics.length > 0) {
		return { valid: false, diagnostics, normalized: null, explanation: null };
	}
	const normalized = normalize(blueprint);
	return { valid: true, diagnostics, normalized, explanation: explain(normalized) };
}
