// Plain words for the formation: the short phrase under each point of a line,
// and the page's summary in full sentences. Locust's own explanation stays
// available word for word under "What Locust will say".

import type {
	Authority,
	Formation,
	CompletionRule,
	EvidenceKind,
	Selector,
	StartRule
} from '../contract/types.ts';
import { stageOrder } from './edit.ts';
import {
	addAnswer,
	afterAnswer,
	countsAnswer,
	kinds,
	lineRules,
	MAIN,
	pickAnswer,
	sameAsMain,
	workAnswer,
	type LineRef,
	type LineRules,
	type PointName,
	type Who
} from './line.ts';

const capital = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);
const quoted = (name: string) => `"${name}"`;

export function shortKey(key: string): string {
	return `${key.slice(0, 8)}…`;
}

/** Who a selector names, as a noun phrase. */
export function who(value: Selector): string {
	switch (value.kind) {
		case 'members':
			return 'any member';
		case 'role':
			return `members in the ${quoted(value.name)} role`;
		case 'participant':
			return `one specific member (key ${shortKey(value.key)})`;
		case 'task_creator':
			return 'the member who added the task';
		case 'contribution_author':
			return 'the author of the result';
		case 'any':
			return value.selectors.map(who).join(' or ');
		case 'nobody':
			return 'nobody';
	}
}

/** Who a single decider is. */
export function decider(value: Authority): string {
	return value.kind === 'role'
		? `the member in the ${quoted(value.name)} role`
		: `one specific member (key ${shortKey(value.key)})`;
}

function startSentence(start: StartRule): string {
	if (start.kind === 'offered') {
		return `${capital(who(start.by))} can ask ${who(start.to)} to do a task. The member asked can say no.`;
	}
	return `${capital(who(start.by))} can start work on a task.`;
}

/** "a result counts when …", without the opening words. */
export function countsClause(rule: CompletionRule): string {
	switch (rule.kind) {
		case 'declaration':
			return rule.by.kind === 'contribution_author'
				? 'its author says so'
				: `${who(rule.by)} says so`;
		case 'contribution':
			return `${who(rule.by)} posts it`;
		case 'reviews': {
			const from =
				rule.by.kind === 'members'
					? ''
					: ` from ${rule.by.kind === 'role' ? quoted(rule.by.name) : who(rule.by)}`;
			return `it has ${rule.count} ${rule.count === 1 ? 'approval' : 'approvals'}${from}${
				rule.exclude_author ? ", not the author's" : ''
			}`;
		}
		case 'check':
			return `the check ${quoted(rule.name)} is reported as passed`;
		case 'all':
			return rule.rules.map(countsClause).join(' and ');
		case 'any':
			return `one of these is true: ${rule.rules.map(countsClause).join('; ')}`;
	}
}

export const EVIDENCE_WORDS: Record<EvidenceKind, (step: string) => string> = {
	publication: (s) => `${quoted(s)} has a posted result`,
	review: (s) => `a result of ${quoted(s)} has an approval`,
	completion: (s) => `${quoted(s)} has a result that counts`,
	selection: (s) => `a result of ${quoted(s)} is picked`
};

const whoWord = (value: Who) => (value.kind === 'anyone' ? 'a member' : quoted(value.name));

/** The question each point answers. */
export const QUESTIONS: Record<PointName, string> = {
	add: 'Who adds tasks?',
	work: 'Who works on a task?',
	counts: 'When does a result count?',
	pick: 'Is one result picked?'
};

/** The short answer shown under a point of a line. */
export function phrase(formation: Formation, ref: LineRef, point: PointName): string {
	const rules = lineRules(formation, ref);
	switch (point) {
		case 'add': {
			if (ref.kind === 'step') return afterPhrase(formation, ref.name);
			if (ref.kind === 'kind') return 'A member, as this kind of task.';
			const answer = addAnswer(rules.work);
			if (answer.kind === 'anyone') return 'Anyone.';
			if (answer.kind === 'role') return `Only ${quoted(answer.name)}.`;
			if (answer.kind === 'none') return 'No tasks. Members only post results.';
			return `${capital(who(rules.work.propose))}.`;
		}
		case 'work': {
			const answer = workAnswer(rules.work);
			if (answer.kind === 'anyone')
				return 'Anyone. No lock: two members can work on the same task.';
			if (answer.kind === 'role') return `Only ${quoted(answer.name)}.`;
			if (answer.kind === 'asks') {
				return `${capital(whoWord(answer.by))} asks ${answer.by.kind === 'anyone' ? 'another member' : 'a member'}. They can say no.`;
			}
			if (answer.kind === 'none') return 'Nobody starts work. Members post results directly.';
			return rules.work.starts.map(startSentence).join(' ');
		}
		case 'counts': {
			const rule = rules.decisions.completion;
			const answer = countsAnswer(rule);
			if (answer.kind === 'own') return `When ${countsClause(rule)}.`;
			const parts: string[] = [];
			if (answer.approvals !== null) {
				const { by, count, excludeAuthor } = answer.approvals;
				parts.push(
					`${count} ${count === 1 ? 'approval' : 'approvals'}${
						by.kind === 'role' ? ` from ${quoted(by.name)}` : ''
					}${excludeAuthor ? ", not the author's" : ''}`
				);
			}
			if (answer.check !== null) {
				parts.push(`the check ${quoted(answer.check.name)} reported as passed`);
			}
			return parts.length === 0 ? 'When its author says so.' : `After ${parts.join(' and ')}.`;
		}
		case 'pick': {
			const { selection } = rules.decisions;
			if (selection === null) return 'Nobody. Every result that counts stays.';
			const answer = pickAnswer(rules.decisions);
			return answer.pick.kind === 'role'
				? `${capital(quoted(answer.pick.name))} picks one result per task.`
				: `${capital(decider(selection))} picks one result per task.`;
		}
	}
}

/** When Locust adds a step's task. */
export function afterPhrase(formation: Formation, step: string): string {
	const answer = afterAnswer(formation, step);
	if (answer.kind === 'start') return 'Locust, at the start.';
	if (answer.kind === 'after') {
		return `Locust, after ${EVIDENCE_WORDS[answer.picked ? 'selection' : 'completion'](answer.step)}.`;
	}
	const requires = formation.flow[step]?.requires ?? [];
	return `Locust, after ${requires.map((item) => EVIDENCE_WORDS[item.evidence](item.stage)).join(' and ')}.`;
}

export interface SummaryLine {
	area: 'roles' | 'rules' | 'steps' | 'kinds' | 'kept';
	text: string;
}

function list(names: string[]): string {
	const names_ = names.map(quoted);
	if (names_.length <= 1) return names_.join('');
	return `${names_.slice(0, -1).join(', ')} and ${names_[names_.length - 1]}`;
}

/** Full sentences for one line's rules. */
function sentences(rules: LineRules, points: PointName[], tasks: string): string[] {
	const out: string[] = [];
	for (const point of points) {
		if (point === 'add') {
			const answer = addAnswer(rules.work);
			out.push(
				answer.kind === 'none'
					? 'Nobody adds tasks. Members only post results.'
					: `${capital(who(rules.work.propose))} can add ${tasks}.`
			);
		}
		if (point === 'work') {
			const answer = workAnswer(rules.work);
			if (answer.kind === 'anyone') {
				out.push(
					'Any member can work on a task. There is no lock: two members can work on the same task.'
				);
			} else if (answer.kind === 'role') {
				out.push(
					`Only members in the ${quoted(answer.name)} role work on a task and post results.`
				);
			} else if (answer.kind === 'asks') {
				const asker =
					answer.by.kind === 'anyone'
						? 'Any member can ask another member'
						: `The ${quoted(answer.by.name)} role can ask a member`;
				out.push(
					`${asker} to do a task. The member asked can say no. Other members can still post results.`
				);
			} else if (answer.kind === 'none') {
				out.push('Nobody starts work on a task. Members post results directly.');
			} else {
				out.push(...rules.work.starts.map(startSentence));
				out.push(`${capital(who(rules.work.publish))} can post results.`);
			}
		}
		if (point === 'counts') {
			out.push(`A result counts when ${countsClause(rules.decisions.completion)}.`);
			if (/check/.test(JSON.stringify(rules.decisions.completion))) {
				out.push('Locust does not run a check. A member reports it.');
			}
		}
		if (point === 'pick') {
			const { selection, finish } = rules.decisions;
			out.push(
				selection === null
					? 'Nobody picks one result. Every result that counts stays.'
					: `${capital(decider(selection))} picks one result per task. The other results stay.`
			);
			if (finish !== null) {
				out.push(`${capital(decider(finish))} can close a task, which stops new work on it.`);
			}
		}
	}
	return out;
}

const POINTS: PointName[] = ['add', 'work', 'counts', 'pick'];

/** The page's plain summary of a formation. */
export function summarize(formation: Formation): SummaryLine[] {
	const lines: SummaryLine[] = [];
	const roles = Object.keys(formation.roles);
	lines.push({
		area: 'roles',
		text:
			roles.length === 0
				? 'There are no roles. Every member takes part on equal terms.'
				: `Roles: ${list(roles)}. Members are put into roles later, in Locust.`
	});
	for (const text of sentences(lineRules(formation, MAIN), POINTS, 'tasks')) {
		lines.push({ area: 'rules', text });
	}
	for (const name of stageOrder(formation)) {
		const ref: LineRef = { kind: 'step', name };
		const own = POINTS.filter((point) => point !== 'add' && !sameAsMain(formation, ref, point));
		const added = afterPhrase(formation, name).replace(/^Locust, /, '');
		const rules = sentences(lineRules(formation, ref), own, 'tasks');
		lines.push({
			area: 'steps',
			text: `Step ${quoted(name)}: Locust adds this task ${added} ${
				rules.length === 0 ? 'It follows the rules for any task.' : rules.join(' ')
			}`
		});
	}
	for (const name of kinds(formation)) {
		const ref: LineRef = { kind: 'kind', name };
		const own = POINTS.filter((point) => !sameAsMain(formation, ref, point));
		const rules = sentences(lineRules(formation, ref), own, `${quoted(name)} tasks`);
		lines.push({
			area: 'kinds',
			text: `Tasks of the kind ${quoted(name)}: ${
				rules.length === 0 ? 'they follow the rules for any task.' : rules.join(' ')
			}`
		});
	}
	const inputs = Object.entries(formation.context.inputs);
	if (inputs.length > 0) {
		const parts = inputs.map(
			([name, input]) =>
				`${quoted(name)} (${input.kind === 'artifact' ? 'a file' : 'text'}${input.required ? '' : ', optional'})`
		);
		lines.push({
			area: 'kept',
			text: `Starting material, kept as it is: ${parts.join(', ')}.`
		});
	}
	if (formation.context.guidance.trim() !== '') {
		lines.push({ area: 'kept', text: 'Advice text, kept as it is. Locust does not show it yet.' });
	}
	return lines;
}
