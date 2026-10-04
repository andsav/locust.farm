// Edits the editor makes to a formation. Each returns a new document and keeps
// every reference consistent: renaming a role, stage or task type rewrites
// every place that names it, and removing one removes what depended on it.

import { compareCodePoints } from '../contract/text.ts';
import type {
	Authority,
	Formation,
	CompletionRule,
	DecisionRules,
	Selector,
	StartRule,
	WorkRules
} from '../contract/types.ts';
import type { EditorDocument } from './document.ts';
import { DEFAULT_WAY, presetFormation } from './presets.ts';

/** A new document holding a way of working. */
export function newDocument(id = DEFAULT_WAY): EditorDocument {
	return { name: '', formation: presetFormation(id), own: {}, others: {} };
}

/** Whether a role, step or kind can have this name. "__proto__" cannot be a key of a plain object. */
export function usableName(name: string): boolean {
	return name !== '' && name !== '__proto__';
}

function clone(document: EditorDocument): EditorDocument {
	return structuredClone(document);
}

/** Loads a way of working; the name is kept. */
export function applyWay(document: EditorDocument, id: string): EditorDocument {
	const next = clone(document);
	next.formation = presetFormation(id);
	next.own = {};
	return next;
}

export function setName(document: EditorDocument, name: string): EditorDocument {
	return { ...clone(document), name };
}

// Walking every selector and authority, to rename or remove a role.

function mapSelector(value: Selector, role: (name: string) => Selector | null): Selector | null {
	if (value.kind === 'role') return role(value.name);
	if (value.kind === 'any') {
		const selectors = value.selectors
			.map((item) => mapSelector(item, role))
			.filter((item): item is Selector => item !== null);
		return selectors.length === 0 ? null : { kind: 'any', selectors };
	}
	return value;
}

function mapCompletion(
	value: CompletionRule,
	role: (name: string) => Selector | null
): CompletionRule {
	if (value.kind === 'all' || value.kind === 'any') {
		return { kind: value.kind, rules: value.rules.map((item) => mapCompletion(item, role)) };
	}
	return { ...value, by: mapSelector(value.by, role) ?? { kind: 'nobody' } };
}

function mapWork(value: WorkRules, role: (name: string) => Selector | null): WorkRules {
	const keep = (selector: Selector) => mapSelector(selector, role) ?? { kind: 'nobody' };
	return {
		propose: keep(value.propose),
		publish: keep(value.publish),
		starts: value.starts.flatMap((start): StartRule[] => {
			const by = mapSelector(start.by, role);
			if (by === null) return [];
			if (start.kind === 'independent') return [{ kind: 'independent' as const, by }];
			const to = mapSelector(start.to, role);
			return to === null ? [] : [{ kind: 'offered' as const, by, to }];
		})
	};
}

function mapAuthority(value: Authority | null, from: string, to: string | null): Authority | null {
	if (value === null || value.kind !== 'role' || value.name !== from) return value;
	return to === null ? null : { kind: 'role', name: to };
}

function mapDecisions(
	value: DecisionRules,
	role: (name: string) => Selector | null,
	from: string,
	to: string | null
): DecisionRules {
	return {
		completion: mapCompletion(value.completion, role),
		selection: mapAuthority(value.selection, from, to),
		finish: mapAuthority(value.finish, from, to)
	};
}

function mapRoles(formation: Formation, from: string, to: string | null): Formation {
	const role = (name: string): Selector | null =>
		name !== from ? { kind: 'role', name } : to === null ? null : { kind: 'role', name: to };
	const next = structuredClone(formation);
	next.work = mapWork(next.work, role);
	next.decisions = mapDecisions(next.decisions, role, from, to);
	for (const taskType of Object.values(next.task_types)) {
		if (taskType.work) taskType.work = mapWork(taskType.work, role);
		if (taskType.decisions) taskType.decisions = mapDecisions(taskType.decisions, role, from, to);
	}
	for (const stage of Object.values(next.flow)) {
		stage.recipients = mapSelector(stage.recipients, role) ?? { kind: 'nobody' };
	}
	return next;
}

/** How many rules name a role, for the confirmation before removing it. */
export function roleUses(formation: Formation, name: string): number {
	let uses = 0;
	const count = (value: unknown) => {
		if (Array.isArray(value)) value.forEach(count);
		else if (typeof value === 'object' && value !== null) {
			const record = value as Record<string, unknown>;
			if (record.kind === 'role' && record.name === name) uses += 1;
			Object.values(record).forEach(count);
		}
	};
	count({ ...formation, roles: {} });
	return uses;
}

export function addRole(document: EditorDocument, name: string, description = ''): EditorDocument {
	if (!usableName(name)) return document;
	const next = clone(document);
	next.formation.roles[name] = { description };
	return next;
}

export function describeRole(
	document: EditorDocument,
	name: string,
	description: string
): EditorDocument {
	const next = clone(document);
	if (Object.hasOwn(next.formation.roles, name)) next.formation.roles[name] = { description };
	return next;
}

export function renameRole(document: EditorDocument, from: string, to: string): EditorDocument {
	if (from === to || !usableName(to) || Object.hasOwn(document.formation.roles, to)) {
		return document;
	}
	const next = clone(document);
	next.formation = mapRoles(next.formation, from, to);
	const roles: Formation['roles'] = {};
	for (const [name, role] of Object.entries(next.formation.roles))
		roles[name === from ? to : name] = role;
	next.formation.roles = roles;
	return next;
}

/**
 * Removes a role. Rules that named only this role are left with nobody allowed
 * to act, never with a broader group, so they show as problems. A step that
 * waited for the role's pick waits for a result that counts instead.
 */
export function removeRole(document: EditorDocument, name: string): EditorDocument {
	const next = clone(document);
	const formation = mapRoles(next.formation, name, null);
	delete formation.roles[name];
	const picks = (stage: string) => {
		const own = formation.flow[stage]?.task_type ?? null;
		const type = own !== null && Object.hasOwn(formation.task_types, own) ? own : null;
		const decisions =
			(type === null ? null : formation.task_types[type].decisions) ?? formation.decisions;
		return decisions.selection !== null;
	};
	for (const stage of Object.values(formation.flow)) {
		for (const item of stage.requires) {
			if (item.evidence === 'selection' && Object.hasOwn(formation.flow, item.stage)) {
				if (!picks(item.stage)) item.evidence = 'completion';
			}
		}
	}
	next.formation = formation;
	return next;
}

// Task types.

export function setTaskType(
	document: EditorDocument,
	name: string,
	taskType: { work: WorkRules | null; decisions: DecisionRules | null } | null
): EditorDocument {
	const next = clone(document);
	if (taskType === null) {
		delete next.formation.task_types[name];
		delete next.own[name];
		for (const stage of Object.values(next.formation.flow)) {
			if (stage.task_type === name) stage.task_type = null;
		}
	} else {
		next.formation.task_types[name] = structuredClone(taskType);
	}
	return next;
}

export function renameTaskType(document: EditorDocument, from: string, to: string): EditorDocument {
	if (from === to || !usableName(to) || Object.hasOwn(document.formation.task_types, to)) {
		return document;
	}
	const next = clone(document);
	if (Object.hasOwn(next.own, from)) {
		next.own[to] = next.own[from];
		delete next.own[from];
	}
	const taskTypes: Formation['task_types'] = {};
	for (const [name, value] of Object.entries(next.formation.task_types)) {
		taskTypes[name === from ? to : name] = value;
	}
	next.formation.task_types = taskTypes;
	for (const stage of Object.values(next.formation.flow)) {
		if (stage.task_type === from) stage.task_type = to;
	}
	return next;
}

// Stages.

/** A name not yet used by a stage or a task type: "step", "step 2", "step 3"… */
export function freeName(formation: Formation, base: string): string {
	const taken = (name: string) =>
		Object.hasOwn(formation.flow, name) || Object.hasOwn(formation.task_types, name);
	if (!taken(base)) return base;
	for (let index = 2; ; index += 1) {
		const name = `${base} ${index}`;
		if (!taken(name)) return name;
	}
}

export function renameStage(document: EditorDocument, from: string, to: string): EditorDocument {
	if (
		from === to ||
		!usableName(to) ||
		Object.hasOwn(document.formation.flow, to) ||
		!Object.hasOwn(document.formation.flow, from)
	) {
		return document;
	}
	let next = clone(document);
	const flow: Formation['flow'] = {};
	for (const [name, stage] of Object.entries(next.formation.flow)) {
		stage.requires = stage.requires.map((item) =>
			item.stage === from ? { ...item, stage: to } : item
		);
		flow[name === from ? to : name] = stage;
	}
	next.formation.flow = flow;
	// A stage's own rules are a task type named after it; the name follows the stage.
	const own = flow[to].task_type;
	const shared = Object.values(flow).filter((stage) => stage.task_type === own).length > 1;
	if (own === from && !shared && !Object.hasOwn(next.formation.task_types, to)) {
		next = renameTaskType(next, from, to);
	}
	return next;
}

/** Removes a stage. Stages that waited for it wait for what it waited for. */
export function removeStage(document: EditorDocument, name: string): EditorDocument {
	if (!Object.hasOwn(document.formation.flow, name)) return document;
	const next = clone(document);
	const removed = next.formation.flow[name];
	delete next.formation.flow[name];
	for (const stage of Object.values(next.formation.flow)) {
		if (!stage.requires.some((item) => item.stage === name)) continue;
		const kept = stage.requires.filter((item) => item.stage !== name);
		for (const item of removed.requires) {
			if (!kept.some((other) => other.stage === item.stage && other.evidence === item.evidence)) {
				kept.push({ ...item });
			}
		}
		stage.requires = kept;
	}
	// Rules only this stage used go with it, so they do not turn up as another kind of task.
	const own = removed.task_type;
	if (
		own !== null &&
		Object.hasOwn(next.formation.task_types, own) &&
		!Object.values(next.formation.flow).some((stage) => stage.task_type === own)
	) {
		delete next.formation.task_types[own];
		delete next.own[own];
	}
	return next;
}

/** Stages that `name` waits for, directly or not. */
function upstreamOf(formation: Formation, name: string): Set<string> {
	const seen = new Set<string>();
	const visit = (stage: string) => {
		for (const item of formation.flow[stage]?.requires ?? []) {
			if (!seen.has(item.stage)) {
				seen.add(item.stage);
				visit(item.stage);
			}
		}
	};
	visit(name);
	return seen;
}

/** Whether "to waits for from" would make a stage wait for itself. */
export function wouldLoop(formation: Formation, from: string, to: string): boolean {
	return from === to || upstreamOf(formation, from).has(to);
}

/** Stage names in the order the page lists them: by what they wait for, then by name. */
export function stageOrder(formation: Formation): string[] {
	const names = Object.keys(formation.flow);
	const depth = new Map<string, number>();
	const level = (name: string, trail: Set<string>): number => {
		if (depth.has(name)) return depth.get(name)!;
		if (trail.has(name)) return 0;
		trail.add(name);
		const requires = formation.flow[name]?.requires ?? [];
		const value = requires.reduce(
			(max, item) =>
				Object.hasOwn(formation.flow, item.stage)
					? Math.max(max, level(item.stage, trail) + 1)
					: max,
			0
		);
		depth.set(name, value);
		return value;
	};
	return names.sort((a, b) => level(a, new Set()) - level(b, new Set()) || compareCodePoints(a, b));
}
