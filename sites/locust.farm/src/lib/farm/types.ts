// Generated from Rust FarmSnapshot by scripts/generate-farm-types.mjs. Do not edit.
export type FarmSnapshot = {
	agents: Array<FarmAgent>;
	attempts: Array<FarmAttempt>;
	candidates: Array<FarmCandidate>;
	changes: Array<FarmChange>;
	farm_id: string;
	formation: string;
	goal_state: FarmGoalState;
	groups: Array<FarmGroup>;
	observed_at_ms?: number | null;
	omitted_changes: number;
	stages: Array<FarmStage>;
	tasks: Array<FarmTask>;
	title?: string | null;
	version: number;
};
export type FarmAgent = {
	group: number;
	harness: Harness;
	id: number;
	name: string;
	roles: Array<string>;
};
export type FarmAttempt = {
	agent: number;
	id: number;
	observed_at_ms?: number | null;
	round: number;
	state: FarmAttemptState;
	task: number;
};
export type FarmAttemptState =
	'started' | 'progress' | 'completed' | 'failed' | 'abandoned' | 'uncertain';
export type FarmCandidate = {
	agent: number;
	completed: boolean;
	evidence_count: number;
	id: number;
	requirement: string;
	round: number;
	selected: boolean;
	task?: number | null;
};
export type FarmChange = {
	agent?: number | null;
	id: number;
	kind: FarmChangeKind;
	observed_at_ms?: number | null;
	task?: number | null;
	text: string;
};
export type FarmChangeKind =
	| 'membership'
	| 'task'
	| 'attempt'
	| 'contribution'
	| 'evidence'
	| 'closure'
	| 'retraction'
	| 'publication';
export type FarmGoalState = 'open' | 'ended' | 'unavailable' | 'disputed';
export type FarmGroup = { id: number; label?: string | null; last_sync_at_ms?: number | null };
export type FarmStage = { id: number; label: string; prerequisites: Array<number> };
export type FarmTask = {
	closed: boolean;
	completed: boolean;
	id: number;
	reference: string;
	round: number;
	selected_candidate?: number | null;
	stage?: number | null;
	state: FarmTaskState;
};
export type FarmTaskState =
	'open' | 'reported' | 'awaiting_evidence' | 'completed' | 'closed' | 'unavailable' | 'disputed';
export type Harness =
	'codex' | 'claude_code' | 'factory_droid' | 'kimi_code' | 'pi' | 'unknown' | 'multiple';
