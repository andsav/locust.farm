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
import type { EditorDocument, PointName } from './document.ts';
import { freeName, stageOrder, wouldLoop } from './edit.ts';

export type { PointName } from './document.ts';

/** Which rules a line shows: the main rules, a step's or another kind of task's. */
export type LineRef =
	{ kind: 'main' } | { kind: 'step'; name: string } | { kind: 'kind'; name: string };

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

/** The alternative that makes a lone member's posted result count. */
export function withoutOnlyMember(rule: CompletionRule): CompletionRule | null {
	if (rule.kind !== 'any' || rule.rules.length !== 2) return null;
	const index = rule.rules.findIndex(
		(part) => part.kind === 'contribution' && part.by.kind === 'only_member'
	);
	return index === -1 ? null : rule.rules[1 - index];
}

function approvalsOf(rule: CompletionRule): Approvals | null {
	rule = withoutOnlyMember(rule) ?? rule;
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
		const review: CompletionRule = {
			kind: 'reviews',
			by: selectorOf(answer.approvals.by),
			count: answer.approvals.count,
			exclude_author: answer.approvals.excludeAuthor
		};
		rules.push(
			answer.approvals.by.kind === 'anyone' && answer.approvals.excludeAuthor
				? { kind: 'any', rules: [review, { kind: 'contribution', by: { kind: 'only_member' } }] }
				: review
		);
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

const POINTS: readonly PointName[] = ['add', 'work', 'counts', 'pick'];

/** The fields of the rules that each point answers. A point is changed and followed as a whole. */
const FIELDS = {
	add: { part: 'work', fields: ['propose'] },
	work: { part: 'work', fields: ['starts', 'publish'] },
	counts: { part: 'decisions', fields: ['completion'] },
	pick: { part: 'decisions', fields: ['selection', 'finish'] }
} as const;

function answerAt(rules: LineRules, point: PointName): unknown[] {
	const { part, fields } = FIELDS[point];
	const values = rules[part] as unknown as Record<string, unknown>;
	return fields.map((field) => values[field]);
}

/** Whether a step or kind gives the same answer as the main rules at one point. */
export function sameAsMain(formation: Formation, ref: LineRef, point: PointName): boolean {
	if (ref.kind === 'main') return true;
	return same(
		answerAt(lineRules(formation, ref), point),
		answerAt(lineRules(formation, MAIN), point)
	);
}

/**
 * Whether a step or kind follows the main rules at one point: it gives the
 * same answer and has not made the point its own. A point made its own stays
 * as it is when the main rules change, even while the two happen to be equal.
 */
export function follows(document: EditorDocument, ref: LineRef, point: PointName): boolean {
	if (ref.kind === 'main') return true;
	const type = typeName(document.formation, ref);
	const marked = type !== null && (document.own[type] ?? []).includes(point);
	return !marked && sameAsMain(document.formation, ref, point);
}

/** Whether a line has no tasks to work on: nobody adds any and Locust adds none as steps. */
export function noTasks(formation: Formation, ref: LineRef): boolean {
	if (ref.kind === 'step') return false;
	if (addAnswer(lineRules(formation, ref).work).kind !== 'none') return false;
	return ref.kind === 'kind' || Object.keys(formation.flow).length === 0;
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

/** A part of a task type that sets nothing of its own is not stored. */
function collapse(type: TaskType, marks: readonly PointName[], main: LineRules) {
	const owns = (part: 'work' | 'decisions') => marks.some((point) => FIELDS[point].part === part);
	if (type.work !== null && !owns('work') && same(type.work, main.work)) type.work = null;
	if (type.decisions !== null && !owns('decisions') && same(type.decisions, main.decisions)) {
		type.decisions = null;
	}
}

/**
 * A task type keeps following the main rules at every point it has not made
 * its own and where it gave the same answer before the change.
 */
function follow(type: TaskType, marks: readonly PointName[], before: LineRules, now: LineRules) {
	for (const point of POINTS) {
		const { part, fields } = FIELDS[point];
		const values = type[part] as unknown as Record<string, unknown> | null;
		if (values === null || marks.includes(point)) continue;
		const old = before[part] as unknown as Record<string, unknown>;
		const current = now[part] as unknown as Record<string, unknown>;
		if (fields.every((field) => same(values[field], old[field]))) {
			for (const field of fields) values[field] = structuredClone(current[field]);
		}
	}
	collapse(type, marks, now);
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
function tidy(document: EditorDocument) {
	const formation = document.formation;
	for (const stage of Object.values(formation.flow)) {
		const own = stage.task_type;
		if (own === null || !Object.hasOwn(formation.task_types, own)) continue;
		const type = formation.task_types[own];
		const shared = Object.values(formation.flow).filter((other) => other.task_type === own);
		if (type.work === null && type.decisions === null && shared.length === 1) {
			stage.task_type = null;
			delete formation.task_types[own];
		}
	}
	for (const name of Object.keys(document.own)) {
		const type = Object.hasOwn(formation.task_types, name) ? formation.task_types[name] : null;
		const points = document.own[name].filter(
			(point) => type !== null && type[FIELDS[point].part] !== null
		);
		if (points.length === 0) delete document.own[name];
		else document.own[name] = points;
	}
}

/**
 * Keeps who a step's task is sent to, and what later steps wait for, in line
 * with the rules. `changed` names the steps whose "who works" was just set.
 */
function settle(before: Formation, formation: Formation, changed: ReadonlySet<string>) {
	for (const [name, stage] of Object.entries(formation.flow)) {
		const ref: LineRef = { kind: 'step', name };
		const earlier = before.flow[name];
		const derived =
			earlier !== undefined &&
			(earlier.recipients.kind === 'nobody' ||
				same(earlier.recipients, recipientsFor(lineRules(before, ref).work)));
		if (changed.has(name) || derived) {
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

/**
 * Writes a line's rules. For a step or kind, each point that changed becomes
 * its own unless it now equals the main rules; `release` gives a point back to
 * the main rules whatever its answer.
 */
function setRules(
	document: EditorDocument,
	ref: LineRef,
	change: Partial<LineRules>,
	release: PointName | null = null
): EditorDocument {
	const next = structuredClone(document);
	const formation = next.formation;
	const main: LineRules = { work: formation.work, decisions: formation.decisions };
	const changed = new Set<string>();
	if (ref.kind === 'main') {
		const now: LineRules = {
			work: structuredClone(change.work ?? main.work),
			decisions: structuredClone(change.decisions ?? main.decisions)
		};
		for (const [name, type] of Object.entries(formation.task_types)) {
			// A point where the type differs from the main rules is its own. Say so
			// before the main rules move, or it would be lost if they came to equal it.
			const line: LineRules = {
				work: type.work ?? main.work,
				decisions: type.decisions ?? main.decisions
			};
			const marks = new Set(next.own[name] ?? []);
			for (const point of POINTS) {
				if (!same(answerAt(line, point), answerAt(main, point))) marks.add(point);
			}
			if (marks.size > 0) next.own[name] = POINTS.filter((point) => marks.has(point));
			follow(type, next.own[name] ?? [], main, now);
		}
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
				if (Object.hasOwn(next.own, current)) next.own[name] = [...next.own[current]];
			} else {
				name = current;
			}
			stage.task_type = name;
			if (change.work) changed.add(ref.name);
		}
		const type = formation.task_types[name];
		const before: LineRules = {
			work: type.work ?? main.work,
			decisions: type.decisions ?? main.decisions
		};
		const now: LineRules = {
			work: structuredClone(change.work ?? before.work),
			decisions: structuredClone(change.decisions ?? before.decisions)
		};
		const marks = new Set(next.own[name] ?? []);
		for (const point of POINTS) {
			if (point === release) marks.delete(point);
			else if (same(answerAt(now, point), answerAt(before, point))) continue;
			else if (same(answerAt(now, point), answerAt(main, point))) marks.delete(point);
			else marks.add(point);
		}
		type.work = now.work;
		type.decisions = now.decisions;
		collapse(type, [...marks], main);
		if (marks.size > 0) next.own[name] = POINTS.filter((point) => marks.has(point));
		else delete next.own[name];
	}
	tidy(next);
	settle(document.formation, formation, changed);
	// A choice that changes nothing is not a change to undo.
	if (same(next.formation, document.formation) && same(next.own, document.own)) return document;
	return next;
}

export function setAdd(
	document: EditorDocument,
	ref: LineRef,
	answer: Exclude<AddAnswer, { kind: 'own' }>
): EditorDocument {
	const work = structuredClone(lineRules(document.formation, ref).work);
	// Who works on a task is left as it is: with "nobody", steps may still add tasks.
	work.propose =
		answer.kind === 'none'
			? { kind: 'nobody' }
			: answer.kind === 'anyone'
				? { kind: 'members' }
				: { kind: 'role', name: answer.name };
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
	const { part, fields } = FIELDS[point];
	const values = line[part] as unknown as Record<string, unknown>;
	const from = main[part] as unknown as Record<string, unknown>;
	for (const field of fields) values[field] = structuredClone(from[field]);
	return setRules(document, ref, { [part]: line[part] }, point);
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
