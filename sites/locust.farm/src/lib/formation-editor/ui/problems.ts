// Plain sentences for Locust's diagnostics, and where "Show me" should go.
// Locust's own message and code stay available under "Technical details".

import type { Formation, Diagnostic } from '../contract/types.ts';
import type { PageCheck } from '../model/checks.ts';
import type { LineRef, PointName } from '../model/line.ts';

/** Where a problem lives on the page: a point of a line, or the roles. */
export type Place = { kind: 'point'; ref: LineRef; point: PointName } | { kind: 'roles' };

export interface Problem {
	text: string;
	place: Place | null;
	blocksCopy: boolean;
	/** Locust's code and wording, or the page's for its own checks. */
	technical: string | null;
	fromLocust: boolean;
}

function unescape(segment: string): string {
	return segment.replaceAll('~1', '/').replaceAll('~0', '~');
}

function pointOf(group: string | undefined, field: string | undefined): PointName {
	if (group === 'work') return field === 'propose' ? 'add' : 'work';
	if (group === 'decisions') return field === 'completion' ? 'counts' : 'pick';
	return 'work';
}

/** Where a JSON Pointer lives on the page. */
export function placeOf(path: string, formation: Formation): Place | null {
	const parts = path.split('/').slice(1).map(unescape);
	switch (parts[0]) {
		case 'roles':
			return { kind: 'roles' };
		case 'work':
		case 'decisions':
			return { kind: 'point', ref: { kind: 'main' }, point: pointOf(parts[0], parts[1]) };
		case 'task_types': {
			const name = parts[1];
			if (name === undefined) return null;
			const step = Object.keys(formation.flow).find((s) => formation.flow[s].task_type === name);
			const ref: LineRef =
				step === undefined ? { kind: 'kind', name } : { kind: 'step', name: step };
			return { kind: 'point', ref, point: pointOf(parts[2], parts[3]) };
		}
		case 'flow': {
			const name = parts[1];
			if (name === undefined || !Object.hasOwn(formation.flow, name)) return null;
			const point: PointName =
				parts[2] === 'recipients' ? 'work' : parts[2] === 'task_type' ? 'counts' : 'add';
			return { kind: 'point', ref: { kind: 'step', name }, point };
		}
		default:
			return null;
	}
}

/** A key for a point of a line, to mark where problems are. */
export function pointKey(ref: LineRef, point: PointName): string {
	return `${ref.kind}:${ref.kind === 'main' ? '' : ref.name}:${point}`;
}

const quoted = (message: string) => /"((?:[^"\\]|\\.)*)"/.exec(message)?.[1] ?? '';

function sentence(diagnostic: Diagnostic, where: Place | null): string {
	const step = where?.kind === 'point' && where.ref.kind === 'step' ? where.ref.name : null;
	const inStage = step === null ? '' : ` in step "${step}"`;
	switch (diagnostic.code) {
		case 'invalid_name':
			return 'A name is empty or contains control characters. Give it a visible name.';
		case 'unknown_role':
			return `A rule${inStage} names the role "${quoted(diagnostic.message)}", which is not in the list of roles. Add the role or choose another.`;
		case 'invalid_participant':
			return `A specific member's key${inStage} is not valid. Keys are 64 hexadecimal characters; a role is usually better.`;
		case 'selector_scope':
			return diagnostic.message.includes('task creator')
				? `"The member who added the task" can only be used for working on or closing tasks, not here.`
				: `"The author of the result" can only be used in a "when does a result count" rule.`;
		case 'empty_selector':
			return 'A choice of members is empty. Pick at least one.';
		case 'impossible_completion':
			return 'No result could ever count with this rule. Choose who approves or reports.';
		case 'invalid_threshold':
			return 'The number of approvals must be at least 1.';
		case 'impossible_threshold':
			return 'This rule asks for more approvals than there can ever be. Lower the number or choose a role.';
		case 'empty_criteria':
			return 'A group of "counts" rules is empty. Tick at least one.';
		case 'unknown_task_type':
			return `Step "${step ?? ''}" follows rules that no longer exist. Choose when its result counts again.`;
		case 'unknown_stage':
			return `A step${inStage} waits for "${quoted(diagnostic.message)}", which is not a step. Choose again when Locust adds it.`;
		case 'unavailable_evidence':
			return `A step${inStage} waits for a picked result, but nobody picks results in the step before it. Decide who picks one there, or choose again when Locust adds this step.`;
		case 'flow_cycle':
			return 'Some steps wait for each other in a loop, so Locust can add none of them.';
		case 'unsupported_version':
			return 'This formation is for a different format of Locust formations.';
		default:
			return diagnostic.message;
	}
}

export function fromDiagnostics(diagnostics: Diagnostic[], formation: Formation): Problem[] {
	return diagnostics.map((diagnostic) => {
		const place = placeOf(diagnostic.path, formation);
		return {
			text: sentence(diagnostic, place),
			place,
			blocksCopy: false,
			technical: `${diagnostic.code} at ${diagnostic.path || 'the whole formation'}: ${diagnostic.message}`,
			fromLocust: true
		};
	});
}

export function fromPageChecks(checks: PageCheck[], formation: Formation): Problem[] {
	return checks.map((check) => ({
		text: check.message,
		place: placeOf(check.path, formation),
		blocksCopy: check.blocksCopy,
		technical: null,
		fromLocust: false
	}));
}

/** The points that have a problem, by pointKey. */
export function problemPoints(problems: Problem[]): Set<string> {
	const out = new Set<string>();
	for (const problem of problems) {
		if (problem.place?.kind === 'point') out.add(pointKey(problem.place.ref, problem.place.point));
	}
	return out;
}
