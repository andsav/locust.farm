// Turns parsed JSON into a typed blueprint, reporting the first structural
// problem at the same JSON Pointer as Locust (serde with serde_path_to_error).
// Path rules, confirmed against the CLI:
// - a wrong value in a plain field is reported at that field;
// - an unknown field is reported at the unknown key;
// - a missing field is reported at the object that lacks it;
// - inside a value tagged by "kind", every problem is reported at the
//   outermost tagged value, except a bad "kind" on the outermost one, which is
//   reported at ".../kind".
// Messages are this page's own; Locust's come from its JSON library.

import { field, isNumber, isObject, type JsonObject, type JsonValue } from './json.ts';
import { escapePointer } from './text.ts';
import {
	defaultDecisions,
	defaultWork,
	type Authority,
	type Blueprint,
	type CompletionRule,
	type Context,
	type DecisionRules,
	type EvidenceKind,
	type Input,
	type InputKind,
	type Prerequisite,
	type Role,
	type Selector,
	type Stage,
	type StartRule,
	type TaskType,
	type WorkRules
} from './types.ts';

export interface StructureError {
	path: string;
	message: string;
}

class Failure extends Error {
	readonly path: string;
	constructor(path: string, message: string) {
		super(message);
		this.path = path;
	}
}

/** Where to report a problem: the field itself, or the outermost tagged value. */
interface Place {
	path: string;
	tagged: string | null;
}

const at = (place: Place) => place.tagged ?? place.path;
const child = (place: Place, key: string | number): Place => ({
	path: `${place.path}/${typeof key === 'number' ? key : escapePointer(key)}`,
	tagged: place.tagged
});

function describe(value: JsonValue): string {
	if (value === null) return 'null';
	if (typeof value === 'boolean') return 'true or false';
	if (typeof value === 'string') return 'text';
	if (Array.isArray(value)) return 'a list';
	return value.type === 'number' ? 'a number' : 'an object';
}

function wrongType(place: Place, value: JsonValue, expected: string): never {
	throw new Failure(at(place), `Expected ${expected}, found ${describe(value)}.`);
}

function string(value: JsonValue, place: Place): string {
	if (typeof value !== 'string') wrongType(place, value, 'text');
	return value as string;
}

function boolean(value: JsonValue, place: Place): boolean {
	if (typeof value !== 'boolean') wrongType(place, value, 'true or false');
	return value as boolean;
}

/** A Rust u32: a whole number from 0 to 4,294,967,295 written without a fraction or exponent. */
function u32(value: JsonValue, place: Place): number {
	if (!isNumber(value) || !/^-?[0-9]+$/.test(value.text)) {
		wrongType(place, value, 'a whole number');
	}
	const number = BigInt((value as { text: string }).text);
	if (number < 0n || number > 4294967295n) {
		throw new Failure(at(place), 'Expected a whole number from 0 to 4294967295.');
	}
	return Number(number);
}

function list(value: JsonValue, place: Place): JsonValue[] {
	if (!Array.isArray(value)) wrongType(place, value, 'a list');
	return value as JsonValue[];
}

function map<T>(
	value: JsonValue,
	place: Place,
	decodeItem: (item: JsonValue, place: Place) => T
): Record<string, T> {
	if (!isObject(value)) wrongType(place, value, 'an object');
	const out: Record<string, T> = Object.create(null);
	for (const [key, item] of (value as JsonObject).entries) {
		out[key] = decodeItem(item, child(place, key));
	}
	return { ...out };
}

/**
 * Reads a struct: visits fields in document order, refuses unknown ones, then
 * reports the first missing required field.
 */
function struct(
	value: JsonValue,
	place: Place,
	expected: string,
	fields: Record<string, (item: JsonValue, place: Place) => void>,
	required: string[] = [],
	ignored: string[] = []
): void {
	if (!isObject(value)) wrongType(place, value, expected);
	const seen = new Set<string>();
	for (const [key, item] of (value as JsonObject).entries) {
		if (ignored.includes(key)) continue;
		const read = fields[key];
		if (!read) {
			const known = Object.keys(fields)
				.map((name) => `"${name}"`)
				.join(', ');
			throw new Failure(at(child(place, key)), `Unknown field "${key}". Known fields: ${known}.`);
		}
		seen.add(key);
		read(item, child(place, key));
	}
	for (const name of required) {
		if (!seen.has(name)) throw new Failure(at(place), `Missing field "${name}".`);
	}
}

/**
 * Reads a value tagged by "kind". Returns the kind and a place whose problems
 * report at the outermost tagged value.
 */
function tagged(
	value: JsonValue,
	place: Place,
	expected: string,
	kinds: readonly string[]
): { kind: string; inner: Place; object: JsonObject } {
	if (!isObject(value)) wrongType(place, value, expected);
	const object = value as JsonObject;
	const tag = field(object, 'kind');
	if (tag === undefined) throw new Failure(at(place), 'Missing field "kind".');
	const kindPlace = place.tagged ? place : { path: `${place.path}/kind`, tagged: null };
	if (typeof tag !== 'string') wrongType(kindPlace, tag, 'a kind name');
	if (!kinds.includes(tag as string)) {
		throw new Failure(
			at(kindPlace),
			`Unknown kind "${tag}". Expected one of: ${kinds.map((kind) => `"${kind}"`).join(', ')}.`
		);
	}
	return { kind: tag as string, inner: { path: place.path, tagged: at(place) }, object };
}

const SELECTOR_KINDS = [
	'members',
	'role',
	'participant',
	'task_creator',
	'contribution_author',
	'any',
	'nobody'
] as const;

function selector(value: JsonValue, place: Place): Selector {
	const { kind, inner } = tagged(value, place, 'a selector object', SELECTOR_KINDS);
	switch (kind) {
		case 'role': {
			let name = '';
			struct(
				value,
				inner,
				'a selector',
				{ name: (v, p) => (name = string(v, p)) },
				['name'],
				['kind']
			);
			return { kind, name };
		}
		case 'participant': {
			let key = '';
			struct(
				value,
				inner,
				'a selector',
				{ key: (v, p) => (key = string(v, p)) },
				['key'],
				['kind']
			);
			return { kind, key };
		}
		case 'any': {
			let selectors: Selector[] = [];
			struct(
				value,
				inner,
				'a selector',
				{ selectors: (v, p) => (selectors = list(v, p).map((s, i) => selector(s, child(p, i)))) },
				['selectors'],
				['kind']
			);
			return { kind, selectors };
		}
		default:
			// Locust ignores extra fields on kinds that have none.
			return { kind } as Selector;
	}
}

function authority(value: JsonValue, place: Place): Authority {
	const { kind, inner } = tagged(value, place, 'an authority object', ['role', 'participant']);
	if (kind === 'role') {
		let name = '';
		struct(
			value,
			inner,
			'an authority',
			{ name: (v, p) => (name = string(v, p)) },
			['name'],
			['kind']
		);
		return { kind, name };
	}
	let key = '';
	struct(value, inner, 'an authority', { key: (v, p) => (key = string(v, p)) }, ['key'], ['kind']);
	return { kind: 'participant', key };
}

function optionalAuthority(value: JsonValue, place: Place): Authority | null {
	return value === null ? null : authority(value, place);
}

function start(value: JsonValue, place: Place): StartRule {
	const { kind, inner } = tagged(value, place, 'a start rule object', ['independent', 'offered']);
	let by: Selector = { kind: 'members' };
	let to: Selector = { kind: 'members' };
	if (kind === 'independent') {
		struct(value, inner, 'a start rule', { by: (v, p) => (by = selector(v, p)) }, ['by'], ['kind']);
		return { kind, by };
	}
	struct(
		value,
		inner,
		'a start rule',
		{ by: (v, p) => (by = selector(v, p)), to: (v, p) => (to = selector(v, p)) },
		['by', 'to'],
		['kind']
	);
	return { kind: 'offered', by, to };
}

const COMPLETION_KINDS = ['contribution', 'declaration', 'reviews', 'check', 'all', 'any'] as const;

function completion(value: JsonValue, place: Place): CompletionRule {
	const { kind, inner } = tagged(value, place, 'a completion rule object', COMPLETION_KINDS);
	let by: Selector = { kind: 'members' };
	const readBy = (v: JsonValue, p: Place) => (by = selector(v, p));
	switch (kind) {
		case 'contribution':
		case 'declaration':
			struct(value, inner, 'a completion rule', { by: readBy }, ['by'], ['kind']);
			return { kind, by };
		case 'reviews': {
			let count = 0;
			let excludeAuthor = true;
			struct(
				value,
				inner,
				'a completion rule',
				{
					by: readBy,
					count: (v, p) => (count = u32(v, p)),
					exclude_author: (v, p) => (excludeAuthor = boolean(v, p))
				},
				['by', 'count'],
				['kind']
			);
			return { kind, by, count, exclude_author: excludeAuthor };
		}
		case 'check': {
			let name = '';
			struct(
				value,
				inner,
				'a completion rule',
				{ name: (v, p) => (name = string(v, p)), by: readBy },
				['name', 'by'],
				['kind']
			);
			return { kind, name, by };
		}
		default: {
			let rules: CompletionRule[] = [];
			struct(
				value,
				inner,
				'a completion rule',
				{ rules: (v, p) => (rules = list(v, p).map((r, i) => completion(r, child(p, i)))) },
				['rules'],
				['kind']
			);
			return { kind: kind as 'all' | 'any', rules };
		}
	}
}

function work(value: JsonValue, place: Place): WorkRules {
	const out = defaultWork();
	struct(value, place, 'an object', {
		propose: (v, p) => (out.propose = selector(v, p)),
		publish: (v, p) => (out.publish = selector(v, p)),
		starts: (v, p) => (out.starts = list(v, p).map((s, i) => start(s, child(p, i))))
	});
	return out;
}

function decisions(value: JsonValue, place: Place): DecisionRules {
	const out = defaultDecisions();
	struct(value, place, 'an object', {
		completion: (v, p) => (out.completion = completion(v, p)),
		selection: (v, p) => (out.selection = optionalAuthority(v, p)),
		finish: (v, p) => (out.finish = optionalAuthority(v, p))
	});
	return out;
}

function role(value: JsonValue, place: Place): Role {
	const out: Role = { description: '' };
	struct(value, place, 'an object', { description: (v, p) => (out.description = string(v, p)) });
	return out;
}

function unitEnum<T extends string>(value: JsonValue, place: Place, names: readonly T[]): T {
	if (typeof value !== 'string') wrongType(place, value, 'a name');
	if (!names.includes(value as T)) {
		throw new Failure(
			at(place),
			`Unknown value "${value}". Expected one of: ${names.map((n) => `"${n}"`).join(', ')}.`
		);
	}
	return value as T;
}

function input(value: JsonValue, place: Place): Input {
	const out: Input = { kind: 'text', required: true };
	struct(
		value,
		place,
		'an object',
		{
			kind: (v, p) => (out.kind = unitEnum<InputKind>(v, p, ['text', 'artifact'])),
			required: (v, p) => (out.required = boolean(v, p))
		},
		['kind']
	);
	return out;
}

function context(value: JsonValue, place: Place): Context {
	const out: Context = { guidance: '', inputs: {} };
	struct(value, place, 'an object', {
		guidance: (v, p) => (out.guidance = string(v, p)),
		inputs: (v, p) => (out.inputs = map(v, p, input))
	});
	return out;
}

function taskType(value: JsonValue, place: Place): TaskType {
	const out: TaskType = { work: null, decisions: null };
	struct(value, place, 'an object', {
		work: (v, p) => (out.work = v === null ? null : work(v, p)),
		decisions: (v, p) => (out.decisions = v === null ? null : decisions(v, p))
	});
	return out;
}

const EVIDENCE: readonly EvidenceKind[] = ['publication', 'review', 'completion', 'selection'];

function prerequisite(value: JsonValue, place: Place): Prerequisite {
	const out: Prerequisite = { stage: '', evidence: 'completion' };
	struct(
		value,
		place,
		'an object',
		{
			stage: (v, p) => (out.stage = string(v, p)),
			evidence: (v, p) => (out.evidence = unitEnum(v, p, EVIDENCE))
		},
		['stage', 'evidence']
	);
	return out;
}

function stage(value: JsonValue, place: Place): Stage {
	const out: Stage = {
		runner: { kind: 'role', name: '' },
		recipients: { kind: 'members' },
		task_type: null,
		requires: []
	};
	struct(
		value,
		place,
		'an object',
		{
			runner: (v, p) => (out.runner = authority(v, p)),
			recipients: (v, p) => (out.recipients = selector(v, p)),
			task_type: (v, p) => (out.task_type = v === null ? null : string(v, p)),
			requires: (v, p) => (out.requires = list(v, p).map((r, i) => prerequisite(r, child(p, i))))
		},
		['runner']
	);
	return out;
}

export function decodeBlueprint(
	value: JsonValue
): { ok: true; blueprint: Blueprint } | { ok: false; error: StructureError } {
	const root: Place = { path: '', tagged: null };
	const out: Blueprint = {
		schema_version: 1,
		roles: {},
		context: { guidance: '', inputs: {} },
		work: defaultWork(),
		decisions: defaultDecisions(),
		task_types: {},
		flow: {}
	};
	try {
		struct(
			value,
			root,
			'a blueprint object',
			{
				schema_version: (v, p) => (out.schema_version = u32(v, p)),
				roles: (v, p) => (out.roles = map(v, p, role)),
				context: (v, p) => (out.context = context(v, p)),
				work: (v, p) => (out.work = work(v, p)),
				decisions: (v, p) => (out.decisions = decisions(v, p)),
				task_types: (v, p) => (out.task_types = map(v, p, taskType)),
				flow: (v, p) => (out.flow = map(v, p, stage))
			},
			['schema_version']
		);
		return { ok: true, blueprint: out };
	} catch (error) {
		if (error instanceof Failure)
			return { ok: false, error: { path: error.path, message: error.message } };
		throw error;
	}
}
