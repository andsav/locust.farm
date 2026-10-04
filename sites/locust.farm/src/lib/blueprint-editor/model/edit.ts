// Edits the editor makes to a blueprint. Each returns a new document and keeps
// every reference consistent: renaming a role, stage or task type rewrites
// every place that names it, and removing one removes what depended on it.

import { compareCodePoints } from '../contract/text.ts';
import type {
	Authority,
	Blueprint,
	CompletionRule,
	DecisionRules,
	EvidenceKind,
	Selector,
	Stage,
	StartRule,
	WorkRules
} from '../contract/types.ts';
import { emptyLayout, type EditorDocument, type Point } from './document.ts';
import { DEFAULT_WAY, presetBlueprint } from './presets.ts';

/** A new document holding a way of working, named after it. */
export function newDocument(id = DEFAULT_WAY): EditorDocument {
	return { name: '', blueprint: presetBlueprint(id), layout: emptyLayout(), way: id };
}

function clone(document: EditorDocument): EditorDocument {
	return structuredClone(document);
}

/** Loads a way of working; the name and other layout keys are kept. */
export function applyWay(document: EditorDocument, id: string): EditorDocument {
	const next = clone(document);
	next.blueprint = presetBlueprint(id);
	next.way = id;
	next.layout.stages = {};
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
		if (stage.runner.kind === 'role' && stage.runner.name === from && to !== null) {
			stage.runner = { kind: 'role', name: to };
		}
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
 * Stages it ran keep the name, which Locust reports as an unknown role.
 */
export function removeRole(document: EditorDocument, name: string): EditorDocument {
	const next = clone(document);
	next.blueprint = mapRoles(next.blueprint, name, null);
	delete next.blueprint.roles[name];
	return next;
}

// Work and decisions at goal level.

export function setWork(document: EditorDocument, work: Partial<WorkRules>): EditorDocument {
	const next = clone(document);
	next.blueprint.work = { ...next.blueprint.work, ...structuredClone(work) };
	return next;
}

export function setDecisions(
	document: EditorDocument,
	decisions: Partial<DecisionRules>
): EditorDocument {
	const next = clone(document);
	next.blueprint.decisions = { ...next.blueprint.decisions, ...structuredClone(decisions) };
	return next;
}

export function setGuidance(document: EditorDocument, guidance: string): EditorDocument {
	const next = clone(document);
	next.blueprint.context.guidance = guidance;
	return next;
}

export function setInput(
	document: EditorDocument,
	name: string,
	input: { kind: 'text' | 'artifact'; required: boolean } | null,
	renameTo?: string
): EditorDocument {
	const next = clone(document);
	const inputs: Blueprint['context']['inputs'] = {};
	for (const [key, value] of Object.entries(next.blueprint.context.inputs)) {
		if (key !== name) inputs[key] = value;
		else if (input !== null) inputs[renameTo ?? key] = input;
	}
	if (input !== null && !Object.hasOwn(next.blueprint.context.inputs, name)) inputs[name] = input;
	next.blueprint.context.inputs = inputs;
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

/** A name not yet used by a stage: "step", "step 2", "step 3"… */
export function freeStageName(blueprint: Blueprint, base = 'step'): string {
	if (!Object.hasOwn(blueprint.flow, base)) return base;
	for (let index = 2; ; index += 1) {
		const name = `${base} ${index}`;
		if (!Object.hasOwn(blueprint.flow, name)) return name;
	}
}

/** The role a new stage is run by: an existing single-person role, or a new "runner" role. */
function defaultRunner(blueprint: Blueprint): { authority: Authority; newRole: string | null } {
	for (const stage of Object.values(blueprint.flow)) {
		if (stage.runner.kind === 'role') return { authority: stage.runner, newRole: null };
	}
	return {
		authority: { kind: 'role', name: 'runner' },
		newRole: Object.hasOwn(blueprint.roles, 'runner') ? null : 'runner'
	};
}

export function addStage(
	document: EditorDocument,
	name: string,
	position: Point | null
): EditorDocument {
	if (Object.hasOwn(document.blueprint.flow, name)) return document;
	const next = clone(document);
	const runner = defaultRunner(next.blueprint);
	if (runner.newRole !== null) {
		next.blueprint.roles[runner.newRole] = {
			description: 'The person whose Locust hands out stage work when it is ready.'
		};
	}
	const stage: Stage = {
		runner: runner.authority,
		recipients: { kind: 'members' },
		task_type: null,
		requires: []
	};
	next.blueprint.flow[name] = stage;
	if (position) next.layout.stages[name] = position;
	return next;
}

export function renameStage(document: EditorDocument, from: string, to: string): EditorDocument {
	if (
		from === to ||
		Object.hasOwn(document.blueprint.flow, to) ||
		!Object.hasOwn(document.blueprint.flow, from)
	) {
		return document;
	}
	const next = clone(document);
	const flow: Blueprint['flow'] = {};
	for (const [name, stage] of Object.entries(next.blueprint.flow)) {
		stage.requires = stage.requires.map((item) =>
			item.stage === from ? { ...item, stage: to } : item
		);
		flow[name === from ? to : name] = stage;
	}
	next.blueprint.flow = flow;
	if (Object.hasOwn(next.layout.stages, from)) {
		next.layout.stages[to] = next.layout.stages[from];
		delete next.layout.stages[from];
	}
	return next;
}

export function removeStage(document: EditorDocument, name: string): EditorDocument {
	const next = clone(document);
	delete next.blueprint.flow[name];
	delete next.layout.stages[name];
	for (const stage of Object.values(next.blueprint.flow)) {
		stage.requires = stage.requires.filter((item) => item.stage !== name);
	}
	return next;
}

export function moveStage(document: EditorDocument, name: string, position: Point): EditorDocument {
	const next = clone(document);
	next.layout.stages[name] = position;
	return next;
}

export function setStage(
	document: EditorDocument,
	name: string,
	change: Partial<Pick<Stage, 'runner' | 'recipients' | 'task_type'>>
): EditorDocument {
	if (!Object.hasOwn(document.blueprint.flow, name)) return document;
	const next = clone(document);
	next.blueprint.flow[name] = { ...next.blueprint.flow[name], ...structuredClone(change) };
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

/** Whether "to starts when from …" would make a stage wait for itself. */
export function wouldLoop(blueprint: Blueprint, from: string, to: string): boolean {
	return from === to || upstreamOf(blueprint, from).has(to);
}

export function addRequirement(
	document: EditorDocument,
	from: string,
	to: string,
	evidence: EvidenceKind
): EditorDocument {
	const flow = document.blueprint.flow;
	if (!Object.hasOwn(flow, from) || !Object.hasOwn(flow, to)) return document;
	if (wouldLoop(document.blueprint, from, to)) return document;
	if (flow[to].requires.some((item) => item.stage === from && item.evidence === evidence)) {
		return document;
	}
	const next = clone(document);
	next.blueprint.flow[to].requires.push({ stage: from, evidence });
	return next;
}

export function removeRequirement(
	document: EditorDocument,
	to: string,
	index: number
): EditorDocument {
	if (!Object.hasOwn(document.blueprint.flow, to)) return document;
	const next = clone(document);
	next.blueprint.flow[to].requires.splice(index, 1);
	return next;
}

export function setRequirement(
	document: EditorDocument,
	to: string,
	index: number,
	change: { stage?: string; evidence?: EvidenceKind }
): EditorDocument {
	if (!Object.hasOwn(document.blueprint.flow, to)) return document;
	const current = document.blueprint.flow[to].requires[index];
	if (!current) return document;
	const stage = change.stage ?? current.stage;
	if (stage !== current.stage && wouldLoop(document.blueprint, stage, to)) return document;
	const next = clone(document);
	next.blueprint.flow[to].requires[index] = {
		stage,
		evidence: change.evidence ?? current.evidence
	};
	return next;
}

/**
 * Gives a stage its own "when is it done" rule. The rule is stored as a task
 * type named after the stage; passing null returns the stage to the default
 * rules and removes that task type if nothing else uses it.
 */
export function setStageCompletion(
	document: EditorDocument,
	name: string,
	completion: CompletionRule | null
): EditorDocument {
	if (!Object.hasOwn(document.blueprint.flow, name)) return document;
	let next = clone(document);
	const stage = next.blueprint.flow[name];
	if (completion === null) {
		const previous = stage.task_type;
		stage.task_type = null;
		if (previous !== null) {
			const stillUsed = Object.values(next.blueprint.flow).some((s) => s.task_type === previous);
			if (!stillUsed && previous === name) next = setTaskType(next, previous, null);
		}
		return next;
	}
	const taskTypeName = stage.task_type ?? name;
	const existing = next.blueprint.task_types[taskTypeName];
	next.blueprint.task_types[taskTypeName] = {
		work: existing?.work ?? null,
		decisions: {
			...(existing?.decisions ?? structuredClone(next.blueprint.decisions)),
			completion: structuredClone(completion)
		}
	};
	stage.task_type = taskTypeName;
	return next;
}

/** Stage names in the order the map and list show them: by position, then name. */
export function stageOrder(document: EditorDocument): string[] {
	const names = Object.keys(document.blueprint.flow);
	const depth = new Map<string, number>();
	const level = (name: string, trail: Set<string>): number => {
		if (depth.has(name)) return depth.get(name)!;
		if (trail.has(name)) return 0;
		trail.add(name);
		const requires = document.blueprint.flow[name]?.requires ?? [];
		const value = requires.reduce(
			(max, item) =>
				Object.hasOwn(document.blueprint.flow, item.stage)
					? Math.max(max, level(item.stage, trail) + 1)
					: max,
			0
		);
		depth.set(name, value);
		return value;
	};
	return names.sort((a, b) => level(a, new Set()) - level(b, new Set()) || compareCodePoints(a, b));
}
