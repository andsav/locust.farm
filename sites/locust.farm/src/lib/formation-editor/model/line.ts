// The four answers every line on the page shows, read from the formation each
// time, and the changes each answer makes. A line is the rules for any task,
// for one step, or for one other kind of task. Answers are never stored
// separately.

import type {
	Authority,
	Formation,
	CompletionRule,
	DecisionRules,
	EvidenceKind,
	Selector,
	TaskType,
	WorkRules
} from '../contract/types.ts';
import type { EditorDocument } from './document.ts';
import { freeName, stageOrder, wouldLoop } from './edit.ts';

/** Which rules a line shows: the main rules, a step's or another kind of task's. */
export type LineRef =
	{ kind: 'main' } | { kind: 'step'; name: string } | { kind: 'kind'; name: string };

export type PointName = 'add' | 'work' | 'counts' | 'pick';

export interface LineRules {
	work: WorkRules;
	decisions: DecisionRules;
}

export const MAIN: LineRef = { kind: 'main' };

/** Compares rules by value, whatever order their fields were written in. */
function same(a: unknown, b: unknown): boolean {
	const ordered = (value: unknown): unknown => {
		if (Array.isArray(value)) return value.map(ordered);
		if (typeof value !== 'object' || value === null) return value;
		const record = value as Record<string, unknown>;
		return Object.fromEntries(
			Object.keys(record)
				.sort()
				.map((key) => [key, ordered(record[key])])
		);
	};
	return JSON.stringify(ordered(a)) === JSON.stringify(ordered(b));
}

function typeName(formation: Formation, ref: LineRef): string | null {
	if (ref.kind === 'main') return null;
	if (ref.kind === 'kind') return ref.name;
	return formation.flow[ref.name]?.task_type ?? null;
}

/** The rules a line follows, with anything it does not set taken from the main rules. */
export function lineRules(formation: Formation, ref: LineRef): LineRules {
	const name = typeName(formation, ref);
	const type =
		name !== null && Object.hasOwn(formation.task_types, name) ? formation.task_types[name] : null;
	return {
		work: type?.work ?? formation.work,
		decisions: type?.decisions ?? formation.decisions
	};
}

// The answers.

/** Everyone in the goal, or the members in one role. */
export type Who = { kind: 'anyone' } | { kind: 'role'; name: string };

function whoOf(selector: Selector): Who | null {
	if (selector.kind === 'members') return { kind: 'anyone' };
	if (selector.kind === 'role') return { kind: 'role', name: selector.name };
	return null;
}

function selectorOf(who: Who): Selector {
	return who.kind === 'anyone' ? { kind: 'members' } : { kind: 'role', name: who.name };
}

/** Who adds tasks. "own" is a rule from elsewhere that the page shows but does not offer. */
export type AddAnswer =
	{ kind: 'anyone' } | { kind: 'role'; name: string } | { kind: 'none' } | { kind: 'own' };

export function addAnswer(work: WorkRules): AddAnswer {
	if (work.propose.kind === 'members') return { kind: 'anyone' };
	if (work.propose.kind === 'role') return { kind: 'role', name: work.propose.name };
	if (work.propose.kind === 'nobody') return { kind: 'none' };
	return { kind: 'own' };
}

/** Who works on a task. "none" means nobody starts work and members post results directly. */
export type WorkAnswer =
	| { kind: 'anyone' }
	| { kind: 'role'; name: string }
	| { kind: 'asks'; by: Who }
	| { kind: 'none' }
	| { kind: 'own' };

export function workAnswer(work: WorkRules): WorkAnswer {
	const { starts, publish } = work;
	if (starts.length === 0) return publish.kind === 'members' ? { kind: 'none' } : { kind: 'own' };
	if (starts.length !== 1) return { kind: 'own' };
	const [start] = starts;
	if (start.kind === 'independent') {
		if (start.by.kind === 'members' && publish.kind === 'members') return { kind: 'anyone' };
		if (start.by.kind === 'role' && publish.kind === 'role' && publish.name === start.by.name) {
			return { kind: 'role', name: start.by.name };
		}
		return { kind: 'own' };
	}
	const by = whoOf(start.by);
	if (by !== null && start.to.kind === 'members' && publish.kind === 'members') {
		return { kind: 'asks', by };
	}
	return { kind: 'own' };
}

export interface Approvals {
	by: Who;
	count: number;
	excludeAuthor: boolean;
}

export interface Check {
	name: string;
	by: Who;
}

/** What a result needs before it counts. With neither, its author says so. */
export type CountsAnswer =
	{ kind: 'list'; approvals: Approvals | null; check: Check | null } | { kind: 'own' };

function approvalsOf(rule: CompletionRule): Approvals | null {
	if (rule.kind !== 'reviews') return null;
	const by = whoOf(rule.by);
	return by === null ? null : { by, count: rule.count, excludeAuthor: rule.exclude_author };
}

function checkOf(rule: CompletionRule): Check | null {
	if (rule.kind !== 'check') return null;
	const by = whoOf(rule.by);
	return by === null ? null : { name: rule.name, by };
}

export function countsAnswer(rule: CompletionRule): CountsAnswer {
	if (rule.kind === 'declaration' && rule.by.kind === 'contribution_author') {
		return { kind: 'list', approvals: null, check: null };
	}
	const approvals = approvalsOf(rule);
	if (approvals !== null) return { kind: 'list', approvals, check: null };
	const check = checkOf(rule);
	if (check !== null) return { kind: 'list', approvals: null, check };
	if (rule.kind === 'all' && rule.rules.length === 2) {
		const both = rule.rules.map(approvalsOf).find((item) => item !== null) ?? null;
		const named = rule.rules.map(checkOf).find((item) => item !== null) ?? null;
		if (both !== null && named !== null) return { kind: 'list', approvals: both, check: named };
	}
	return { kind: 'own' };
}

export function countsRule(answer: Extract<CountsAnswer, { kind: 'list' }>): CompletionRule {
	const rules: CompletionRule[] = [];
	if (answer.approvals !== null) {
		rules.push({
			kind: 'reviews',
			by: selectorOf(answer.approvals.by),
			count: answer.approvals.count,
			exclude_author: answer.approvals.excludeAuthor
		});
	}
	if (answer.check !== null) {
		rules.push({ kind: 'check', name: answer.check.name, by: selectorOf(answer.check.by) });
	}
	if (rules.length === 0) return { kind: 'declaration', by: { kind: 'contribution_author' } };
	return rules.length === 1 ? rules[0] : { kind: 'all', rules };
}

/** One role, nobody, or a specific person named by a formation from elsewhere. */
export type Decider = { kind: 'nobody' } | { kind: 'role'; name: string } | { kind: 'own' };

function deciderOf(authority: Authority | null): Decider {
	if (authority === null) return { kind: 'nobody' };
	return authority.kind === 'role' ? { kind: 'role', name: authority.name } : { kind: 'own' };
}

/** Who picks one result per task, and who can close a task. */
export interface PickAnswer {
	pick: Decider;
	close: Decider;
}

export function pickAnswer(decisions: DecisionRules): PickAnswer {
	return { pick: deciderOf(decisions.selection), close: deciderOf(decisions.finish) };
}

/** When Locust adds a step's task. */
export type AfterAnswer =
	{ kind: 'start' } | { kind: 'after'; step: string; picked: boolean } | { kind: 'own' };

export function afterAnswer(formation: Formation, step: string): AfterAnswer {
	const requires = formation.flow[step]?.requires ?? [];
	if (requires.length === 0) return { kind: 'start' };
	if (requires.length === 1) {
		const [{ stage, evidence }] = requires;
		if (evidence === 'completion') return { kind: 'after', step: stage, picked: false };
		if (evidence === 'selection') return { kind: 'after', step: stage, picked: true };
	}
	return { kind: 'own' };
}

/** Whether a step or kind gives the same answer as the main rules at one point. */
export function sameAsMain(formation: Formation, ref: LineRef, point: PointName): boolean {
	if (ref.kind === 'main') return true;
	const line = lineRules(formation, ref);
	const main = lineRules(formation, MAIN);
	switch (point) {
		case 'add':
			return same(line.work.propose, main.work.propose);
		case 'work':
			return same([line.work.starts, line.work.publish], [main.work.starts, main.work.publish]);
		case 'counts':
			return same(line.decisions.completion, main.decisions.completion);
		case 'pick':
			return same(
				[line.decisions.selection, line.decisions.finish],
				[main.decisions.selection, main.decisions.finish]
			);
	}
}

// The changes.

/** Who a stage's ready task is sent to, given who works on it. */
function recipientsFor(work: WorkRules): Selector {
	const answer = workAnswer(work);
	return answer.kind === 'role' ? { kind: 'role', name: answer.name } : { kind: 'members' };
}

/** What a later step waits for: a picked result when someone picks, a result that counts otherwise. */
function evidenceFor(formation: Formation, step: string): EvidenceKind {
	const picker = lineRules(formation, { kind: 'step', name: step }).decisions.selection;
	return picker === null ? 'completion' : 'selection';
}

/** A task type keeps following the main rules wherever it matched them before the change. */
function follow(type: TaskType, before: LineRules, now: LineRules) {
	if (type.work !== null) {
		const work = type.work as unknown as Record<string, unknown>;
		for (const field of ['propose', 'publish', 'starts'] as const) {
			if (same(work[field], before.work[field])) work[field] = structuredClone(now.work[field]);
		}
		if (same(type.work, now.work)) type.work = null;
	}
	if (type.decisions !== null) {
		const decisions = type.decisions as unknown as Record<string, unknown>;
		for (const field of ['completion', 'selection', 'finish'] as const) {
			if (same(decisions[field], before.decisions[field])) {
				decisions[field] = structuredClone(now.decisions[field]);
			}
		}
		if (same(type.decisions, now.decisions)) type.decisions = null;
	}
}

/** The name for a step's own rules: the step's name, or the next free one. */
function ownTypeName(formation: Formation, step: string): string {
	if (!Object.hasOwn(formation.task_types, step)) return step;
	for (let index = 2; ; index += 1) {
		const name = `${step} ${index}`;
		if (!Object.hasOwn(formation.task_types, name)) return name;
	}
}

/** Steps whose rules equal the main rules again need no rules of their own. */
function tidy(formation: Formation) {
	for (const [name, stage] of Object.entries(formation.flow)) {
		const own = stage.task_type;
		if (own === null || !Object.hasOwn(formation.task_types, own)) continue;
		const type = formation.task_types[own];
		const shared = Object.values(formation.flow).filter((other) => other.task_type === own);
		if (type.work === null && type.decisions === null && shared.length === 1 && own === name) {
			stage.task_type = null;
			delete formation.task_types[own];
		}
	}
}

/** Keeps who a step's task is sent to, and what later steps wait for, in line with the rules. */
function settle(before: Formation, formation: Formation) {
	for (const [name, stage] of Object.entries(formation.flow)) {
		const ref: LineRef = { kind: 'step', name };
		const earlier = before.flow[name];
		if (earlier && same(earlier.recipients, recipientsFor(lineRules(before, ref).work))) {
			stage.recipients = recipientsFor(lineRules(formation, ref).work);
		}
		const had = earlier ? evidenceFor(before, name) : null;
		const has = evidenceFor(formation, name);
		if (had === null || had === has) continue;
		for (const later of Object.values(formation.flow)) {
			for (const item of later.requires) {
				if (item.stage === name && item.evidence === had) item.evidence = has;
			}
		}
	}
}

function setRules(document: EditorDocument, ref: LineRef, change: Partial<LineRules>) {
	const next = structuredClone(document);
	const formation = next.formation;
	const main: LineRules = { work: formation.work, decisions: formation.decisions };
	if (ref.kind === 'main') {
		const now: LineRules = {
			work: structuredClone(change.work ?? main.work),
			decisions: structuredClone(change.decisions ?? main.decisions)
		};
		for (const type of Object.values(formation.task_types)) follow(type, main, now);
		formation.work = now.work;
		formation.decisions = now.decisions;
	} else {
		let name: string;
		if (ref.kind === 'kind') {
			if (!Object.hasOwn(formation.task_types, ref.name)) return document;
			name = ref.name;
		} else {
			const stage = formation.flow[ref.name];
			if (!stage) return document;
			const current = stage.task_type;
			const shared = Object.values(formation.flow).filter((s) => s.task_type === current);
			if (current === null || !Object.hasOwn(formation.task_types, current)) {
				name = ownTypeName(formation, ref.name);
				formation.task_types[name] = { work: null, decisions: null };
			} else if (shared.length > 1) {
				// Other steps follow the same rules; this step gets a copy to change.
				name = ownTypeName(formation, ref.name);
				formation.task_types[name] = structuredClone(formation.task_types[current]);
			} else {
				name = current;
			}
			stage.task_type = name;
		}
		const type = formation.task_types[name];
		if (change.work) type.work = same(change.work, main.work) ? null : structuredClone(change.work);
		if (change.decisions) {
			type.decisions = same(change.decisions, main.decisions)
				? null
				: structuredClone(change.decisions);
		}
	}
	tidy(formation);
	settle(document.formation, formation);
	return next;
}

export function setAdd(
	document: EditorDocument,
	ref: LineRef,
	answer: Exclude<AddAnswer, { kind: 'own' }>
): EditorDocument {
	const before = lineRules(document.formation, ref).work;
	const work = structuredClone(before);
	if (answer.kind === 'none') {
		work.propose = { kind: 'nobody' };
		work.starts = [];
		work.publish = { kind: 'members' };
	} else {
		work.propose =
			answer.kind === 'anyone' ? { kind: 'members' } : { kind: 'role', name: answer.name };
		// Tasks exist again, so someone has to be able to work on them.
		if (addAnswer(before).kind === 'none' && work.starts.length === 0) {
			work.starts = [{ kind: 'independent', by: { kind: 'members' } }];
		}
	}
	return setRules(document, ref, { work });
}

export function setWork(
	document: EditorDocument,
	ref: LineRef,
	answer: Extract<WorkAnswer, { kind: 'anyone' | 'role' | 'asks' }>
): EditorDocument {
	const work = structuredClone(lineRules(document.formation, ref).work);
	if (answer.kind === 'anyone') {
		work.starts = [{ kind: 'independent', by: { kind: 'members' } }];
		work.publish = { kind: 'members' };
	} else if (answer.kind === 'role') {
		work.starts = [{ kind: 'independent', by: { kind: 'role', name: answer.name } }];
		work.publish = { kind: 'role', name: answer.name };
	} else {
		work.starts = [{ kind: 'offered', by: selectorOf(answer.by), to: { kind: 'members' } }];
		work.publish = { kind: 'members' };
	}
	return setRules(document, ref, { work });
}

export function setCounts(
	document: EditorDocument,
	ref: LineRef,
	answer: Extract<CountsAnswer, { kind: 'list' }>
): EditorDocument {
	const decisions = structuredClone(lineRules(document.formation, ref).decisions);
	decisions.completion = countsRule(answer);
	return setRules(document, ref, { decisions });
}

type Chosen = Exclude<Decider, { kind: 'own' }>;

const authorityOf = (decider: Chosen): Authority | null =>
	decider.kind === 'nobody' ? null : { kind: 'role', name: decider.name };

export function setPick(
	document: EditorDocument,
	ref: LineRef,
	change: { pick?: Chosen; close?: Chosen }
): EditorDocument {
	const decisions = structuredClone(lineRules(document.formation, ref).decisions);
	if (change.pick) decisions.selection = authorityOf(change.pick);
	if (change.close) decisions.finish = authorityOf(change.close);
	return setRules(document, ref, { decisions });
}

/** Makes one point of a step or kind follow the main rules again. */
export function useMain(document: EditorDocument, ref: LineRef, point: PointName): EditorDocument {
	if (ref.kind === 'main') return document;
	const line = structuredClone(lineRules(document.formation, ref));
	const main = lineRules(document.formation, MAIN);
	switch (point) {
		case 'add':
			line.work.propose = main.work.propose;
			return setRules(document, ref, { work: line.work });
		case 'work':
			line.work.starts = main.work.starts;
			line.work.publish = main.work.publish;
			return setRules(document, ref, { work: line.work });
		case 'counts':
			line.decisions.completion = main.decisions.completion;
			return setRules(document, ref, { decisions: line.decisions });
		case 'pick':
			line.decisions.selection = main.decisions.selection;
			line.decisions.finish = main.decisions.finish;
			return setRules(document, ref, { decisions: line.decisions });
	}
}

// Steps and kinds of task.

/** Adds a step after the last one. Returns the new document and the step's name. */
export function addStep(document: EditorDocument): { document: EditorDocument; name: string } {
	const next = structuredClone(document);
	const formation = next.formation;
	const last = stageOrder(formation).at(-1) ?? null;
	const name = freeName(formation, 'step');
	formation.flow[name] = {
		recipients: recipientsFor(formation.work),
		task_type: null,
		requires: last === null ? [] : [{ stage: last, evidence: evidenceFor(formation, last) }]
	};
	return { document: next, name };
}

/** Sets the one step a step waits for, or none. */
export function setAfter(
	document: EditorDocument,
	step: string,
	after: string | null
): EditorDocument {
	const flow = document.formation.flow;
	if (!Object.hasOwn(flow, step)) return document;
	if (
		after !== null &&
		(!Object.hasOwn(flow, after) || wouldLoop(document.formation, after, step))
	) {
		return document;
	}
	const next = structuredClone(document);
	next.formation.flow[step].requires =
		after === null ? [] : [{ stage: after, evidence: evidenceFor(next.formation, after) }];
	return next;
}

/** The task types no step uses: other kinds of task a member can add. */
export function kinds(formation: Formation): string[] {
	const used = new Set(Object.values(formation.flow).map((stage) => stage.task_type));
	return Object.keys(formation.task_types)
		.filter((name) => !used.has(name))
		.sort();
}

/** Adds another kind of task that follows the main rules until a point is changed. */
export function addKind(document: EditorDocument): { document: EditorDocument; name: string } {
	const next = structuredClone(document);
	const name = freeName(next.formation, 'kind');
	next.formation.task_types[name] = { work: null, decisions: null };
	return { document: next, name };
}
