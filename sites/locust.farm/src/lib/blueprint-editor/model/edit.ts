// Edits the editor makes to a blueprint. Each returns a new document and keeps
// every reference consistent: renaming a role, stage or task type rewrites
// every place that names it, and removing one removes what depended on it.

import { compareCodePoints } from '../contract/text.ts';
import type {
	Authority,
	Blueprint,
	CompletionRule,
	DecisionRules,
	Selector,
	StartRule,
	WorkRules
} from '../contract/types.ts';
import type { EditorDocument } from './document.ts';
import { DEFAULT_WAY, presetBlueprint } from './presets.ts';

/** A new document holding a way of working. */
export function newDocument(id = DEFAULT_WAY): EditorDocument {
	return { name: '', blueprint: presetBlueprint(id), others: {} };
}

function clone(document: EditorDocument): EditorDocument {
	return structuredClone(document);
}

/** Loads a way of working; the name is kept. */
export function applyWay(document: EditorDocument, id: string): EditorDocument {
	const next = clone(document);
	next.blueprint = presetBlueprint(id);
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

function mapRoles(blueprint: Blueprint, from: string, to: string | null): Blueprint {
	const role = (name: string): Selector | null =>
		name !== from ? { kind: 'role', name } : to === null ? null : { kind: 'role', name: to };
	const next = structuredClone(blueprint);
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
export function roleUses(blueprint: Blueprint, name: string): number {
	let uses = 0;
	const count = (value: unknown) => {
		if (Array.isArray(value)) value.forEach(count);
		else if (typeof value === 'object' && value !== null) {
			const record = value as Record<string, unknown>;
			if (record.kind === 'role' && record.name === name) uses += 1;
			Object.values(record).forEach(count);
		}
	};
	count({ ...blueprint, roles: {} });
	return uses;
}

export function addRole(document: EditorDocument, name: string, description = ''): EditorDocument {
	const next = clone(document);
	next.blueprint.roles[name] = { description };
	return next;
}

export function describeRole(
	document: EditorDocument,
	name: string,
	description: string
): EditorDocument {
	const next = clone(document);
	if (Object.hasOwn(next.blueprint.roles, name)) next.blueprint.roles[name] = { description };
	return next;
}

export function renameRole(document: EditorDocument, from: string, to: string): EditorDocument {
	if (from === to || Object.hasOwn(document.blueprint.roles, to)) return document;
	const next = clone(document);
	next.blueprint = mapRoles(next.blueprint, from, to);
	const roles: Blueprint['roles'] = {};
	for (const [name, role] of Object.entries(next.blueprint.roles))
		roles[name === from ? to : name] = role;
	next.blueprint.roles = roles;
	return next;
}

/**
 * Removes a role. Rules that named only this role are left with nobody allowed
 * to act, never with a broader group, so Locust reports them as problems.
 */
export function removeRole(document: EditorDocument, name: string): EditorDocument {
	const next = clone(document);
	next.blueprint = mapRoles(next.blueprint, name, null);
	delete next.blueprint.roles[name];
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
		delete next.blueprint.task_types[name];
		for (const stage of Object.values(next.blueprint.flow)) {
			if (stage.task_type === name) stage.task_type = null;
		}
	} else {
		next.blueprint.task_types[name] = structuredClone(taskType);
	}
	return next;
}

export function renameTaskType(document: EditorDocument, from: string, to: string): EditorDocument {
	if (from === to || Object.hasOwn(document.blueprint.task_types, to)) return document;
	const next = clone(document);
	const taskTypes: Blueprint['task_types'] = {};
	for (const [name, value] of Object.entries(next.blueprint.task_types)) {
		taskTypes[name === from ? to : name] = value;
	}
	next.blueprint.task_types = taskTypes;
	for (const stage of Object.values(next.blueprint.flow)) {
		if (stage.task_type === from) stage.task_type = to;
	}
	return next;
}

// Stages.

/** A name not yet used by a stage or a task type: "step", "step 2", "step 3"… */
export function freeName(blueprint: Blueprint, base: string): string {
	const taken = (name: string) =>
		Object.hasOwn(blueprint.flow, name) || Object.hasOwn(blueprint.task_types, name);
	if (!taken(base)) return base;
	for (let index = 2; ; index += 1) {
		const name = `${base} ${index}`;
		if (!taken(name)) return name;
	}
}

export function renameStage(document: EditorDocument, from: string, to: string): EditorDocument {
	if (
		from === to ||
		Object.hasOwn(document.blueprint.flow, to) ||
		!Object.hasOwn(document.blueprint.flow, from)
	) {
		return document;
	}
	let next = clone(document);
	const flow: Blueprint['flow'] = {};
	for (const [name, stage] of Object.entries(next.blueprint.flow)) {
		stage.requires = stage.requires.map((item) =>
			item.stage === from ? { ...item, stage: to } : item
		);
		flow[name === from ? to : name] = stage;
	}
	next.blueprint.flow = flow;
	// A stage's own rules are a task type named after it; the name follows the stage.
	const own = flow[to].task_type;
	const shared = Object.values(flow).filter((stage) => stage.task_type === own).length > 1;
	if (own === from && !shared && !Object.hasOwn(next.blueprint.task_types, to)) {
		next = renameTaskType(next, from, to);
	}
	return next;
}

/** Removes a stage. Stages that waited for it wait for what it waited for. */
export function removeStage(document: EditorDocument, name: string): EditorDocument {
	if (!Object.hasOwn(document.blueprint.flow, name)) return document;
	const next = clone(document);
	const removed = next.blueprint.flow[name];
	delete next.blueprint.flow[name];
	for (const stage of Object.values(next.blueprint.flow)) {
		if (!stage.requires.some((item) => item.stage === name)) continue;
		const kept = stage.requires.filter((item) => item.stage !== name);
		for (const item of removed.requires) {
			if (!kept.some((other) => other.stage === item.stage && other.evidence === item.evidence)) {
				kept.push({ ...item });
			}
		}
		stage.requires = kept;
	}
	const own = removed.task_type;
	if (
		own === name &&
		Object.hasOwn(next.blueprint.task_types, own) &&
		!Object.values(next.blueprint.flow).some((stage) => stage.task_type === own)
	) {
		delete next.blueprint.task_types[own];
	}
	return next;
}

/** Stages that `name` waits for, directly or not. */
function upstreamOf(blueprint: Blueprint, name: string): Set<string> {
	const seen = new Set<string>();
	const visit = (stage: string) => {
		for (const item of blueprint.flow[stage]?.requires ?? []) {
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
export function wouldLoop(blueprint: Blueprint, from: string, to: string): boolean {
	return from === to || upstreamOf(blueprint, from).has(to);
}

/** Stage names in the order the page lists them: by what they wait for, then by name. */
export function stageOrder(blueprint: Blueprint): string[] {
	const names = Object.keys(blueprint.flow);
	const depth = new Map<string, number>();
	const level = (name: string, trail: Set<string>): number => {
		if (depth.has(name)) return depth.get(name)!;
		if (trail.has(name)) return 0;
		trail.add(name);
		const requires = blueprint.flow[name]?.requires ?? [];
		const value = requires.reduce(
			(max, item) =>
				Object.hasOwn(blueprint.flow, item.stage)
					? Math.max(max, level(item.stage, trail) + 1)
					: max,
			0
		);
		depth.set(name, value);
		return value;
	};
	return names.sort((a, b) => level(a, new Set()) - level(b, new Set()) || compareCodePoints(a, b));
}
