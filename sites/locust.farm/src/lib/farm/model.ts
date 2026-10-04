import type { FarmSnapshot, FarmTask, FarmChange } from './types.ts';
import { serviceNow, type FarmState } from './client.ts';

export const harnessName = (harness: string) =>
	({
		codex: 'Codex',
		claude_code: 'Claude Code',
		pi: 'Pi',
		unknown: 'Harness unknown',
		multiple: 'Multiple harnesses'
	})[harness] ?? 'Harness unknown';
export const statusName = (status: string) =>
	({
		open: 'Open',
		reported: 'Attempted',
		awaiting_evidence: 'Awaiting evidence',
		completed: 'Completed',
		closed: 'Closed',
		unavailable: 'Unavailable',
		disputed: 'Disputed'
	})[status] ?? status;
export const mapState = (task: FarmTask) =>
	(
		({ reported: 'taken', awaiting_evidence: 'waiting', completed: 'done' }) as Record<
			string,
			string
		>
	)[task.state] ?? task.state;
export function farmMode(
	state: FarmState,
	now: number
): 'receiving' | 'quiet' | 'ended' | 'unavailable' {
	if (!state.snapshot || state.status === 'unavailable') return 'unavailable';
	if (state.snapshot.goal_state === 'ended') return 'ended';
	return state.received_at_ms != null &&
		serviceNow(state, now) - state.received_at_ms >= 0 &&
		serviceNow(state, now) - state.received_at_ms <= 120000
		? 'receiving'
		: 'quiet';
}
export const modeName = (mode: string) =>
	({ receiving: 'Receiving updates', quiet: 'Quiet', ended: 'Ended', unavailable: 'Unavailable' })[
		mode
	] ?? mode;
export function stageColumns(snapshot: FarmSnapshot): number[][] {
	const levels = new Map<number, number>();
	const pending = [...snapshot.stages].sort((a, b) => a.id - b.id);
	while (pending.length) {
		const index = pending.findIndex((stage) => stage.prerequisites.every((id) => levels.has(id)));
		if (index < 0) break;
		const [stage] = pending.splice(index, 1);
		levels.set(stage.id, Math.max(-1, ...stage.prerequisites.map((id) => levels.get(id)!)) + 1);
	}
	const columns: number[][] = [];
	for (const stage of snapshot.stages) {
		const level = levels.get(stage.id) ?? 0;
		(columns[level] ??= []).push(stage.id);
	}
	for (const column of columns) column.sort((a, b) => a - b);
	if (!columns.length) columns.push([0]);
	return columns;
}
export function associations(snapshot: FarmSnapshot, stage: number | null) {
	return snapshot.agents.flatMap((agent) => {
		const attempts = snapshot.tasks
			.filter((task) => (task.stage ?? null) === stage)
			.flatMap((task) =>
				snapshot.attempts
					.filter(
						(attempt) =>
							attempt.agent === agent.id && attempt.task === task.id && attempt.round === task.round
					)
					.map((attempt) => ({ task, attempt }))
			);
		return attempts.length ? [{ agent, attempts }] : [];
	});
}
export function changeText(change: FarmChange): string {
	return change.text;
}
export function taskFacts(task: FarmTask): string[] {
	return [
		`${statusName(task.state)} · round ${task.round}`,
		`${task.completed ? 'Complete' : 'Not complete'} · ${task.closed ? 'closed' : 'open'} · ${task.selected_candidate == null ? 'no result selected' : `candidate ${task.selected_candidate} selected`}`
	];
}
