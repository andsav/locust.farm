// The answers to the page's questions, read from the blueprint every time, and
// the changes each answer makes. Answers are never stored separately.

import type { CompletionRule, Selector, StartRule } from '../contract/types.ts';
import { blueprintData, type EditorDocument } from './document.ts';
import { setDecisions, setWork } from './edit.ts';

export type StartAnswer =
	{ kind: 'anyone' } | { kind: 'handed-out'; by: Selector } | { kind: 'nobody' } | { kind: 'own' };

export function startAnswer(starts: StartRule[]): StartAnswer {
	if (starts.length === 0) return { kind: 'nobody' };
	if (starts.length === 1) {
		const [start] = starts;
		if (start.kind === 'independent' && start.by.kind === 'members') return { kind: 'anyone' };
		if (start.kind === 'offered' && start.to.kind === 'members')
			return { kind: 'handed-out', by: start.by };
	}
	return { kind: 'own' };
}

export function setStartAnswer(document: EditorDocument, answer: StartAnswer): EditorDocument {
	switch (answer.kind) {
		case 'anyone':
			return setWork(document, { starts: [{ kind: 'independent', by: { kind: 'members' } }] });
		case 'handed-out':
			return setWork(document, {
				starts: [{ kind: 'offered', by: answer.by, to: { kind: 'members' } }]
			});
		case 'nobody':
			return setWork(document, { starts: [] });
		default:
			return document;
	}
}

export type DoneAnswer =
	| { kind: 'self' }
	| { kind: 'review'; by: Selector; count: number; excludeAuthor: boolean }
	| { kind: 'check'; name: string; by: Selector }
	| { kind: 'own' };

export function doneAnswer(rule: CompletionRule): DoneAnswer {
	if (rule.kind === 'declaration' && rule.by.kind === 'contribution_author')
		return { kind: 'self' };
	if (rule.kind === 'reviews') {
		return { kind: 'review', by: rule.by, count: rule.count, excludeAuthor: rule.exclude_author };
	}
	if (rule.kind === 'check') return { kind: 'check', name: rule.name, by: rule.by };
	return { kind: 'own' };
}

export function doneRule(answer: DoneAnswer, current: CompletionRule): CompletionRule {
	switch (answer.kind) {
		case 'self':
			return { kind: 'declaration', by: { kind: 'contribution_author' } };
		case 'review':
			return {
				kind: 'reviews',
				by: answer.by,
				count: answer.count,
				exclude_author: answer.excludeAuthor
			};
		case 'check':
			return { kind: 'check', name: answer.name, by: answer.by };
		default:
			return current;
	}
}

export function setDoneAnswer(document: EditorDocument, answer: DoneAnswer): EditorDocument {
	const current = document.blueprint.decisions.completion;
	const next = doneRule(answer, current);
	return next === current ? document : setDecisions(document, { completion: next });
}

/** Which areas differ from the way of working the document started from. Compared in a fixed key order. */
export function changedFromWay(document: EditorDocument, way: EditorDocument | null): string[] {
	if (way === null) return [];
	const a = blueprintData(document.blueprint) as Record<string, Record<string, unknown>>;
	const b = blueprintData(way.blueprint) as Record<string, Record<string, unknown>>;
	const same = (x: unknown, y: unknown) => JSON.stringify(x) === JSON.stringify(y);
	const changed: string[] = [];
	if (!same(Object.keys(a.roles), Object.keys(b.roles))) changed.push('roles');
	if (!same(a.work, b.work)) changed.push('work');
	if (!same(a.decisions.completion, b.decisions.completion)) changed.push('done');
	if (!same(a.decisions.selection, b.decisions.selection)) changed.push('final answer');
	if (!same(a.decisions.finish, b.decisions.finish)) changed.push('finish');
	if (!same(a.context.guidance, b.context.guidance)) changed.push('advice');
	if (!same(a.context.inputs, b.context.inputs)) changed.push('starting material');
	if (!same(a.task_types, b.task_types)) changed.push('task types');
	if (!same(a.flow, b.flow)) changed.push('stages');
	return changed;
}
