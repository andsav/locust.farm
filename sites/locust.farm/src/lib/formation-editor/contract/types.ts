// The formation format, as Locust's Rust types define it
// (crates/locust-proto/src/organization.rs). Field order follows Rust, which
// matters for sorting and for the normalized form.

export type Selector =
	| { kind: 'members' }
	| { kind: 'role'; name: string }
	| { kind: 'participant'; key: string }
	| { kind: 'task_creator' }
	| { kind: 'contribution_author' }
	| { kind: 'any'; selectors: Selector[] }
	| { kind: 'nobody' }
	| { kind: 'only_member' };

/** A single decider: a role that must have exactly one member, or one participant. */
export type Authority = { kind: 'role'; name: string } | { kind: 'participant'; key: string };

export type StartRule =
	{ kind: 'independent'; by: Selector } | { kind: 'offered'; by: Selector; to: Selector };

export type CompletionRule =
	| { kind: 'contribution'; by: Selector }
	| { kind: 'declaration'; by: Selector }
	| { kind: 'reviews'; by: Selector; count: number; exclude_author: boolean }
	| { kind: 'check'; name: string; by: Selector }
	| { kind: 'all'; rules: CompletionRule[] }
	| { kind: 'any'; rules: CompletionRule[] };

export type InputKind = 'text' | 'artifact';
export type EvidenceKind = 'publication' | 'review' | 'completion' | 'selection';

export interface Role {
	description: string;
}

export interface Input {
	kind: InputKind;
	required: boolean;
}

export interface Context {
	guidance: string;
	inputs: Record<string, Input>;
}

export interface WorkRules {
	propose: Selector;
	publish: Selector;
	starts: StartRule[];
}

export interface DecisionRules {
	completion: CompletionRule;
	selection: Authority | null;
	finish: Authority | null;
}

export interface WorkspacePolicy {
	integrator: Authority;
	completion: CompletionRule;
}

export interface TaskType {
	work: WorkRules | null;
	decisions: DecisionRules | null;
}

export interface Prerequisite {
	stage: string;
	evidence: EvidenceKind;
}

export interface Stage {
	recipients: Selector;
	task_type: string | null;
	requires: Prerequisite[];
}

export interface Formation {
	schema_version: number;
	roles: Record<string, Role>;
	context: Context;
	work: WorkRules;
	decisions: DecisionRules;
	workspace: WorkspacePolicy | null;
	task_types: Record<string, TaskType>;
	flow: Record<string, Stage>;
}

export interface Diagnostic {
	code: string;
	severity: 'error';
	phase: 'load' | 'definition';
	/** RFC 6901 JSON Pointer; an empty string names the whole document. */
	path: string;
	message: string;
	correction: string;
	related_paths: string[];
}

export interface Explanation {
	summary: string[];
	required_roles: string[];
	required_inputs: string[];
	authority_roles: string[];
	contextual_checks: string[];
}

export interface Inspection {
	valid: boolean;
	diagnostics: Diagnostic[];
	normalized: Formation | null;
	explanation: Explanation | null;
}

export const SCHEMA_VERSION = 2;

export function defaultWork(): WorkRules {
	return {
		propose: { kind: 'members' },
		publish: { kind: 'members' },
		starts: [{ kind: 'independent', by: { kind: 'members' } }]
	};
}

export function defaultDecisions(): DecisionRules {
	return {
		completion: { kind: 'declaration', by: { kind: 'contribution_author' } },
		selection: null,
		finish: null
	};
}
