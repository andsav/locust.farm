// What the editor holds: the formation and its name. The name is presentation
// only. It travels in Locust's presentation record under the page's own key and
// never changes the formation.

import { compareCodePoints } from '../contract/text.ts';
import type {
	Authority,
	Formation,
	CompletionRule,
	DecisionRules,
	Selector,
	StartRule,
	WorkRules
} from '../contract/types.ts';

/** The four points of a line: who adds a task, who works on it, when a result counts, whether one is picked. */
export type PointName = 'add' | 'work' | 'counts' | 'pick';

const POINT_NAMES: readonly PointName[] = ['add', 'work', 'counts', 'pick'];

export interface EditorDocument {
	/** A short name, kept in the presentation record so the agent can name the draft. */
	name: string;
	formation: Formation;
	/**
	 * The points a step or kind answers for itself, by task type name. The
	 * formation alone cannot say this once the main rules come to give the same
	 * answer, so it is kept beside the name.
	 */
	own: Record<string, PointName[]>;
	/** Keys other tools wrote into the same presentation record; kept as they are. */
	others: Record<string, unknown>;
}

export const LAYOUT_KEY = 'locust.farm';

/** The presentation record's JSON object for this document. */
export function presentation(document: EditorDocument): Record<string, unknown> {
	const own: Record<string, PointName[]> = {};
	for (const name of Object.keys(document.own).sort(compareCodePoints)) {
		const points = POINT_NAMES.filter((point) => document.own[name].includes(point));
		if (points.length > 0 && Object.hasOwn(document.formation.task_types, name)) own[name] = points;
	}
	const page = Object.keys(own).length > 0 ? { name: document.name, own } : { name: document.name };
	return { ...document.others, [LAYOUT_KEY]: page };
}

/** Reads a presentation record, keeping other tools' keys. */
export function readPresentation(value: unknown): {
	others: Record<string, unknown>;
	name: string | null;
	own: Record<string, PointName[]>;
} {
	const others: Record<string, unknown> = {};
	const own: Record<string, PointName[]> = {};
	let name: string | null = null;
	if (typeof value !== 'object' || value === null || Array.isArray(value)) {
		return { others, name, own };
	}
	for (const [key, item] of Object.entries(value)) {
		if (key !== LAYOUT_KEY) {
			others[key] = item;
			continue;
		}
		if (typeof item !== 'object' || item === null) continue;
		const page = item as Record<string, unknown>;
		if (typeof page.name === 'string') name = page.name;
		if (typeof page.own === 'object' && page.own !== null && !Array.isArray(page.own)) {
			for (const [type, points] of Object.entries(page.own)) {
				if (type === '__proto__' || !Array.isArray(points)) continue;
				const known = POINT_NAMES.filter((point) => points.includes(point));
				if (known.length > 0) own[type] = known;
			}
		}
	}
	return { others, name, own };
}

// Serialization in Rust field order, with map keys in code point order, so the
// same formation always produces the same bytes.

function selector(value: Selector): unknown {
	switch (value.kind) {
		case 'role':
			return { kind: value.kind, name: value.name };
		case 'participant':
			return { kind: value.kind, key: value.key };
		case 'any':
			return { kind: value.kind, selectors: value.selectors.map(selector) };
		default:
			return { kind: value.kind };
	}
}

function authority(value: Authority | null): unknown {
	if (value === null) return null;
	return value.kind === 'role'
		? { kind: value.kind, name: value.name }
		: { kind: value.kind, key: value.key };
}

function start(value: StartRule): unknown {
	return value.kind === 'offered'
		? { kind: value.kind, by: selector(value.by), to: selector(value.to) }
		: { kind: value.kind, by: selector(value.by) };
}

function completion(value: CompletionRule): unknown {
	switch (value.kind) {
		case 'contribution':
		case 'declaration':
			return { kind: value.kind, by: selector(value.by) };
		case 'reviews':
			return {
				kind: value.kind,
				by: selector(value.by),
				count: value.count,
				exclude_author: value.exclude_author
			};
		case 'check':
			return { kind: value.kind, name: value.name, by: selector(value.by) };
		default:
			return { kind: value.kind, rules: value.rules.map(completion) };
	}
}

function work(value: WorkRules): unknown {
	return {
		propose: selector(value.propose),
		publish: selector(value.publish),
		starts: value.starts.map(start)
	};
}

function decisions(value: DecisionRules): unknown {
	return {
		completion: completion(value.completion),
		selection: authority(value.selection),
		finish: authority(value.finish)
	};
}

function sortedRecord<T, U>(record: Record<string, T>, map: (value: T) => U): Record<string, U> {
	const out: Record<string, U> = {};
	for (const key of Object.keys(record).sort(compareCodePoints)) out[key] = map(record[key]);
	return out;
}

/** The formation as plain JSON data in a stable order. */
export function formationData(value: Formation): Record<string, unknown> {
	return {
		schema_version: value.schema_version,
		roles: sortedRecord(value.roles, (role) => ({ description: role.description })),
		context: {
			guidance: value.context.guidance,
			inputs: sortedRecord(value.context.inputs, (input) => ({
				kind: input.kind,
				required: input.required
			}))
		},
		work: work(value.work),
		decisions: decisions(value.decisions),
		task_types: sortedRecord(value.task_types, (taskType) => ({
			work: taskType.work && work(taskType.work),
			decisions: taskType.decisions && decisions(taskType.decisions)
		})),
		flow: sortedRecord(value.flow, (stage) => ({
			recipients: selector(stage.recipients),
			task_type: stage.task_type,
			requires: stage.requires.map((item) => ({ stage: item.stage, evidence: item.evidence }))
		}))
	};
}

// Control, format (bidirectional and zero-width), separator, private-use and
// tag characters are written as \u escapes, so pasted text stays readable and
// no line of a formation can be mistaken for something else. The value is
// unchanged.
const ESCAPE = /[\p{Cc}\p{Cf}\p{Zl}\p{Zp}\u0085\u{E0000}-\u{E007F}]/gu;

/** Two-space indented JSON with LF line endings and no final newline. */
export function toText(value: unknown): string {
	return JSON.stringify(value, null, 2).replace(ESCAPE, (char) => {
		if (char === '\n' || char === '\r' || char === '\t') return char;
		const code = char.codePointAt(0)!;
		if (code <= 0xffff) return `\\u${code.toString(16).padStart(4, '0')}`;
		const high = Math.floor((code - 0x10000) / 0x400) + 0xd800;
		const low = ((code - 0x10000) % 0x400) + 0xdc00;
		return `\\u${high.toString(16)}\\u${low.toString(16)}`;
	});
}

/** The exact text the prompt and downloads carry for the formation. */
export function formationText(value: Formation): string {
	return toText(formationData(value));
}
