// Plain sentences for Locust's diagnostics, and where "Show me" should go.
// Locust's own message and code stay available under "Technical details".

import type { Diagnostic } from '../contract/types.ts';
import type { PageCheck } from '../model/checks.ts';

export type Place =
	| { kind: 'stage'; name: string }
	| {
			kind: 'section';
			id:
				| 'roles'
				| 'work'
				| 'sharing'
				| 'done'
				| 'final-answer'
				| 'finish'
				| 'advice'
				| 'material'
				| 'task-types';
	  };

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

/** Where a JSON Pointer lives in the editor. */
export function placeOf(path: string): Place | null {
	const parts = path.split('/').slice(1).map(unescape);
	switch (parts[0]) {
		case 'flow':
			return parts[1] === undefined ? null : { kind: 'stage', name: parts[1] };
		case 'roles':
			return { kind: 'section', id: 'roles' };
		case 'work':
			return {
				kind: 'section',
				id: parts[1] === 'propose' || parts[1] === 'publish' ? 'sharing' : 'work'
			};
		case 'decisions':
			if (parts[1] === 'selection') return { kind: 'section', id: 'final-answer' };
			if (parts[1] === 'finish') return { kind: 'section', id: 'finish' };
			return { kind: 'section', id: 'done' };
		case 'task_types':
			return { kind: 'section', id: 'task-types' };
		case 'context':
			return { kind: 'section', id: parts[1] === 'guidance' ? 'advice' : 'material' };
		default:
			return null;
	}
}

const quoted = (message: string) => /"((?:[^"\\]|\\.)*)"/.exec(message)?.[1] ?? '';

function sentence(diagnostic: Diagnostic): string {
	const where = placeOf(diagnostic.path);
	const inStage = where?.kind === 'stage' ? ` in stage "${where.name}"` : '';
	switch (diagnostic.code) {
		case 'invalid_name':
			return 'A name is empty or contains control characters. Give it a visible name.';
		case 'unknown_role':
			return `A rule${inStage} names the role "${quoted(diagnostic.message)}", which is not in the list of roles. Add the role or choose another.`;
		case 'invalid_participant':
			return `A specific person's key${inStage} is not valid. Keys are 64 hexadecimal characters; a role is usually better.`;
		case 'selector_scope':
			return diagnostic.message.includes('task creator')
				? `"The person who created the task" can only be used for starting or finishing tasks, not here.`
				: `"The author of the work" can only be used in a "when is a task done" rule.`;
		case 'empty_selector':
			return 'A choice of people is empty. Pick at least one, or choose "nobody".';
		case 'impossible_completion':
			return 'Nobody could ever finish a task with this rule. Choose who can say it is done.';
		case 'invalid_threshold':
			return 'The number of reviewers must be at least 1.';
		case 'impossible_threshold':
			return 'This rule asks for more reviewers than there can ever be. Lower the number or choose a role.';
		case 'empty_criteria':
			return 'A group of "done" rules is empty. Add a rule or remove the group.';
		case 'unknown_task_type':
			return `Stage "${where?.kind === 'stage' ? where.name : ''}" follows rules that no longer exist. Choose its done rule again.`;
		case 'unknown_stage':
			return `A stage${inStage} waits for "${quoted(diagnostic.message)}", which is not a stage. Remove that condition.`;
		case 'unavailable_evidence':
			return `A stage${inStage} waits for a picked result, but nobody picks results in the stage before it. Wait for "is complete" instead, or decide who picks one.`;
		case 'flow_cycle':
			return 'Some stages wait for each other in a loop, so none of them can start.';
		case 'unsupported_version':
			return 'This blueprint is for a different format of Locust blueprints.';
		default:
			return diagnostic.message;
	}
}

export function fromDiagnostics(diagnostics: Diagnostic[]): Problem[] {
	return diagnostics.map((diagnostic) => ({
		text: sentence(diagnostic),
		place: placeOf(diagnostic.path),
		blocksCopy: false,
		technical: `${diagnostic.code} at ${diagnostic.path || 'the whole blueprint'}: ${diagnostic.message}`,
		fromLocust: true
	}));
}

export function fromPageChecks(checks: PageCheck[]): Problem[] {
	return checks.map((check) => ({
		text: check.message,
		place: placeOf(check.path),
		blocksCopy: check.blocksCopy,
		technical: null,
		fromLocust: false
	}));
}

/** Problem counts per stage, for the map. */
export function perStage(problems: Problem[]): Record<string, number> {
	const out: Record<string, number> = {};
	for (const problem of problems) {
		if (problem.place?.kind === 'stage')
			out[problem.place.name] = (out[problem.place.name] ?? 0) + 1;
	}
	return out;
}
