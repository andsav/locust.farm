// Plain words for the blueprint, used by the page's summary, the questions and
// the stage map. Locust's own explanation stays available word for word under
// "What Locust will say".

import type {
	Authority,
	Blueprint,
	CompletionRule,
	EvidenceKind,
	Selector,
	StartRule
} from '../contract/types.ts';
import { stageOrder } from './edit.ts';
import type { EditorDocument } from './document.ts';

const capital = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

export function shortKey(key: string): string {
	return `${key.slice(0, 8)}…`;
}

/** Who a selector names, as a noun phrase. */
export function who(value: Selector): string {
	switch (value.kind) {
		case 'members':
			return 'anyone in the goal';
		case 'role':
			return `people in the "${value.name}" role`;
		case 'participant':
			return `a specific person (key ${shortKey(value.key)})`;
		case 'task_creator':
			return 'the person who created the task';
		case 'contribution_author':
			return 'the author of the work';
		case 'any':
			return value.selectors.map(who).join(' or ');
		case 'nobody':
			return 'nobody';
	}
}

/** Who a single decider is. */
export function decider(value: Authority): string {
	return value.kind === 'role'
		? `the person in the "${value.name}" role`
		: `a specific person (key ${shortKey(value.key)})`;
}

function reviewers(by: Selector, count: number): string {
	const many = count > 1;
	switch (by.kind) {
		case 'members':
			return many ? `${count} different people in the goal` : 'someone in the goal';
		case 'role':
			return many
				? `${count} different people in the "${by.name}" role`
				: `someone in the "${by.name}" role`;
		default:
			return many ? `${count} different people from ${who(by)}` : who(by);
	}
}

export function startSentence(start: StartRule): string {
	if (start.kind === 'offered') {
		return `${capital(who(start.by))} can hand out work to ${who(start.to)}. The person asked has to accept before starting.`;
	}
	if (start.by.kind === 'members') {
		return 'Anyone in the goal can start working on a task. Several people may work on the same task.';
	}
	return `${capital(who(start.by))} can start working on a task.`;
}

/** "a task is done when …", without the opening words. */
export function doneClause(rule: CompletionRule): string {
	switch (rule.kind) {
		case 'declaration':
			return rule.by.kind === 'contribution_author'
				? 'the person who did it says so'
				: `${who(rule.by)} says so`;
		case 'contribution':
			return `${who(rule.by)} shares a result for it`;
		case 'reviews':
			return `${reviewers(rule.by, rule.count)} ${rule.count > 1 ? 'approve' : 'approves'} it${
				rule.exclude_author ? ', not counting the author' : ''
			}`;
		case 'check':
			return `the check "${rule.name}" passes, as reported by ${who(rule.by)}`;
		case 'all':
			return `all of these are true: ${rule.rules.map(doneClause).join('; ')}`;
		case 'any':
			return `any of these is true: ${rule.rules.map(doneClause).join('; ')}`;
	}
}

export const EVIDENCE_WORDS: Record<
	EvidenceKind,
	{ short: string; when: (stage: string) => string }
> = {
	publication: {
		short: 'has a published result',
		when: (s) => `"${s}" has a published result`
	},
	review: { short: 'has a review', when: (s) => `"${s}" has a review` },
	completion: { short: 'is complete', when: (s) => `"${s}" is complete` },
	selection: {
		short: 'has a picked result',
		when: (s) => `a result of "${s}" is picked`
	}
};

export const EVIDENCE_ORDER: readonly EvidenceKind[] = [
	'completion',
	'publication',
	'review',
	'selection'
];

export interface SummaryLine {
	area: 'people' | 'work' | 'done' | 'decisions' | 'advice' | 'material' | 'task types' | 'stages';
	text: string;
}

function list(names: string[]): string {
	const quoted = names.map((name) => `"${name}"`);
	if (quoted.length <= 1) return quoted.join('');
	return `${quoted.slice(0, -1).join(', ')} and ${quoted[quoted.length - 1]}`;
}

/** The page's plain summary of a blueprint. */
export function summarize(document: EditorDocument): SummaryLine[] {
	const value: Blueprint = document.blueprint;
	const lines: SummaryLine[] = [];
	const roles = Object.keys(value.roles);
	lines.push({
		area: 'people',
		text:
			roles.length === 0
				? 'Everyone in the goal takes part on equal terms.'
				: `Roles: ${list(roles)}. You choose who fills each role when you start a goal.`
	});
	const { propose, publish, starts } = value.work;
	if (propose.kind === 'members' && publish.kind === 'members') {
		lines.push({ area: 'work', text: 'Anyone in the goal can suggest tasks and share findings.' });
	} else {
		lines.push({ area: 'work', text: `${capital(who(propose))} can suggest tasks.` });
		lines.push({ area: 'work', text: `${capital(who(publish))} can share findings.` });
	}
	if (starts.length === 0) {
		lines.push({
			area: 'work',
			text: 'Nobody can start new work on tasks. People can still share findings.'
		});
	}
	for (const start of starts) lines.push({ area: 'work', text: startSentence(start) });
	lines.push({
		area: 'done',
		text: `A task is done when ${doneClause(value.decisions.completion)}.`
	});
	const { selection, finish } = value.decisions;
	lines.push({
		area: 'decisions',
		text: selection
			? `${capital(decider(selection))} picks one final answer. Approval alone doesn't pick one.`
			: 'Every result that is done is kept. Nobody picks a single final answer.'
	});
	lines.push({
		area: 'decisions',
		text: finish
			? `${capital(decider(finish))} can say the whole goal is finished.`
			: 'Nobody can say the goal is finished. It stays open.'
	});
	if (value.context.guidance.trim() !== '') {
		lines.push({ area: 'advice', text: 'There is advice for everyone. It is advice, not a rule.' });
	}
	const inputs = Object.entries(value.context.inputs);
	if (inputs.length > 0) {
		const parts = inputs.map(
			([name, input]) =>
				`"${name}" (${input.kind === 'artifact' ? 'a file' : 'text'}${input.required ? '' : ', optional'})`
		);
		lines.push({
			area: 'material',
			text: `Starting material: ${parts.join(', ')}. You provide it when you start a goal.`
		});
	}
	for (const [name, taskType] of Object.entries(value.task_types)) {
		const usedByStage = Object.values(value.flow).some((stage) => stage.task_type === name);
		if (usedByStage) continue;
		const completion = (taskType.decisions ?? value.decisions).completion;
		lines.push({
			area: 'task types',
			text: `Tasks of type "${name}" are done when ${doneClause(completion)}.`
		});
	}
	for (const name of stageOrder(document)) {
		const stage = value.flow[name];
		const starts =
			stage.requires.length === 0
				? `Stage "${name}" can start any time.`
				: `Stage "${name}" starts when ${stage.requires
						.map((item) => EVIDENCE_WORDS[item.evidence].when(item.stage))
						.join(' and ')}.`;
		const done =
			stage.task_type !== null && Object.hasOwn(value.task_types, stage.task_type)
				? ` It is done when ${doneClause((value.task_types[stage.task_type].decisions ?? value.decisions).completion)}.`
				: '';
		lines.push({
			area: 'stages',
			text: `${starts} Its work goes to ${who(stage.recipients)}.${done} The Locust of ${decider(stage.runner)} hands it out.`
		});
	}
	return lines;
}
