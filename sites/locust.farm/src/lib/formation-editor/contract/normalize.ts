// Locust's normalization (crates/locust-core/src/organization/normalize.rs).
// Lists that Locust treats as sets are sorted by their binary encoding
// (postcard): a value tagged by "kind" encodes its kind as length-prefixed
// text and then its fields in Rust order; evidence kinds encode as an index.
// That is why shorter kind and role names sort first.

import type {
	Authority,
	Formation,
	CompletionRule,
	DecisionRules,
	Prerequisite,
	Selector,
	StartRule,
	WorkRules
} from './types.ts';

const utf8 = new TextEncoder();

class Encoder {
	readonly bytes: number[] = [];
	varint(value: number) {
		let rest = value;
		while (rest >= 0x80) {
			this.bytes.push((rest & 0x7f) | 0x80);
			rest = Math.floor(rest / 0x80);
		}
		this.bytes.push(rest);
	}
	text(value: string) {
		const encoded = utf8.encode(value);
		this.varint(encoded.length);
		for (const byte of encoded) this.bytes.push(byte);
	}
}

function encodeSelector(out: Encoder, value: Selector) {
	out.text(value.kind);
	if (value.kind === 'role') out.text(value.name);
	if (value.kind === 'participant') out.text(value.key);
	if (value.kind === 'any') {
		out.varint(value.selectors.length);
		for (const item of value.selectors) encodeSelector(out, item);
	}
}

function encodeStart(out: Encoder, value: StartRule) {
	out.text(value.kind);
	encodeSelector(out, value.by);
	if (value.kind === 'offered') encodeSelector(out, value.to);
}

function encodeCompletion(out: Encoder, value: CompletionRule) {
	out.text(value.kind);
	switch (value.kind) {
		case 'contribution':
		case 'declaration':
			encodeSelector(out, value.by);
			break;
		case 'reviews':
			encodeSelector(out, value.by);
			out.varint(value.count);
			out.bytes.push(value.exclude_author ? 1 : 0);
			break;
		case 'check':
			out.text(value.name);
			encodeSelector(out, value.by);
			break;
		default:
			out.varint(value.rules.length);
			for (const item of value.rules) encodeCompletion(out, item);
	}
}

const EVIDENCE_INDEX = { publication: 0, review: 1, completion: 2, selection: 3 } as const;

function encodePrerequisite(out: Encoder, value: Prerequisite) {
	out.text(value.stage);
	out.varint(EVIDENCE_INDEX[value.evidence]);
}

function compareBytes(a: number[], b: number[]): number {
	const length = Math.min(a.length, b.length);
	for (let index = 0; index < length; index += 1) {
		if (a[index] !== b[index]) return a[index] - b[index];
	}
	return a.length - b.length;
}

/** Sorts by encoding and removes adjacent duplicates, as Locust's `set` does. */
function asSet<T>(values: T[], encode: (out: Encoder, value: T) => void): T[] {
	const keyed = values.map((value) => {
		const out = new Encoder();
		encode(out, value);
		return { value, key: out.bytes };
	});
	keyed.sort((a, b) => compareBytes(a.key, b.key));
	const result: T[] = [];
	let previous: number[] | null = null;
	for (const { value, key } of keyed) {
		if (previous === null || compareBytes(previous, key) !== 0) result.push(value);
		previous = key;
	}
	return result;
}

function selector(value: Selector): Selector {
	if (value.kind === 'participant') return { kind: 'participant', key: value.key.toLowerCase() };
	if (value.kind !== 'any') return value;
	const flattened: Selector[] = [];
	for (const item of value.selectors.map(selector)) {
		if (item.kind === 'any') flattened.push(...item.selectors);
		else flattened.push(item);
	}
	const set = asSet(flattened, encodeSelector);
	return set.length === 1 ? set[0] : { kind: 'any', selectors: set };
}

function authority(value: Authority): Authority {
	return value.kind === 'participant'
		? { kind: 'participant', key: value.key.toLowerCase() }
		: value;
}

function work(value: WorkRules): WorkRules {
	return {
		propose: selector(value.propose),
		publish: selector(value.publish),
		starts: asSet(
			value.starts.map((start): StartRule =>
				start.kind === 'offered'
					? { kind: 'offered', by: selector(start.by), to: selector(start.to) }
					: { kind: 'independent', by: selector(start.by) }
			),
			encodeStart
		)
	};
}

function completion(value: CompletionRule): CompletionRule {
	if (value.kind === 'all' || value.kind === 'any') {
		const rules = asSet(value.rules.map(completion), encodeCompletion);
		return rules.length === 1 ? rules[0] : { kind: value.kind, rules };
	}
	return { ...value, by: selector(value.by) };
}

function decisions(value: DecisionRules): DecisionRules {
	return {
		completion: completion(value.completion),
		selection: value.selection && authority(value.selection),
		finish: value.finish && authority(value.finish)
	};
}

/** Returns a normalized copy; the input is not changed. */
export function normalize(value: Formation): Formation {
	const goalWork = work(value.work);
	const goalDecisions = decisions(value.decisions);
	const taskTypes: Formation['task_types'] = {};
	for (const [name, taskType] of Object.entries(value.task_types)) {
		taskTypes[name] = {
			work: taskType.work ? work(taskType.work) : goalWork,
			decisions: taskType.decisions ? decisions(taskType.decisions) : goalDecisions
		};
	}
	const flow: Formation['flow'] = {};
	for (const [name, stage] of Object.entries(value.flow)) {
		flow[name] = {
			recipients: selector(stage.recipients),
			task_type: stage.task_type,
			requires: asSet(stage.requires, encodePrerequisite)
		};
	}
	return {
		schema_version: value.schema_version,
		roles: value.roles,
		context: value.context,
		work: goalWork,
		decisions: goalDecisions,
		task_types: taskTypes,
		flow
	};
}
