// Locust's explanation lines (crates/locust-core/src/organization/explanation.rs),
// word for word. The agent shows these lines when it explains a blueprint, so the
// page shows the same text under "What Locust will say".

import { references } from './rules.ts';
import { compareCodePoints, rustDebug, sortedKeys } from './text.ts';
import type {
	Authority,
	Blueprint,
	CompletionRule,
	DecisionRules,
	Explanation,
	Selector,
	WorkRules
} from './types.ts';

function selector(value: Selector): string {
	switch (value.kind) {
		case 'members':
			return 'goal members';
		case 'role':
			return `members bound to role ${rustDebug(value.name)}`;
		case 'participant':
			return `participant ${value.key}`;
		case 'task_creator':
			return 'the task creator';
		case 'contribution_author':
			return 'the contribution author';
		case 'any':
			return value.selectors.map(selector).join(' or ');
		case 'nobody':
			return 'nobody';
	}
}

function authority(value: Authority): string {
	return value.kind === 'role'
		? `the single member bound to role ${rustDebug(value.name)}`
		: `participant ${value.key}`;
}

function completion(value: CompletionRule): string {
	switch (value.kind) {
		case 'contribution':
			return `publication by ${selector(value.by)}`;
		case 'declaration':
			return `a completion declaration by ${selector(value.by)}`;
		case 'reviews':
			return `${value.count} distinct approving reviewer(s) from ${selector(value.by)}, ${
				value.exclude_author
					? 'excluding the contribution author'
					: 'including the author if eligible'
			}`;
		case 'check':
			return `check ${rustDebug(value.name)} attested by ${selector(value.by)} on the exact contribution`;
		case 'all':
			return `all of [${value.rules.map(completion).join('; ')}]`;
		case 'any':
			return `any of [${value.rules.map(completion).join('; ')}]`;
	}
}

function work(value: WorkRules, prefix: string, lines: string[]) {
	lines.push(
		`${prefix}: ${selector(value.propose)} may propose tasks; ${selector(value.publish)} may publish contributions without a task.`
	);
	if (value.starts.length === 0) lines.push(`${prefix}: no new task attempts are permitted.`);
	for (const start of value.starts) {
		lines.push(
			start.kind === 'independent'
				? `${prefix}: ${selector(start.by)} may start independent attempts; concurrent attempts may coexist.`
				: `${prefix}: ${selector(start.by)} may offer work to ${selector(start.to)}; the recipient must acknowledge before starting.`
		);
	}
}

function decisions(value: DecisionRules, prefix: string, lines: string[]) {
	lines.push(
		`${prefix}: completion requires ${completion(value.completion)} on the exact candidate.`
	);
	lines.push(
		value.selection
			? `${prefix}: ${authority(value.selection)} may select one qualifying output; approval alone does not select it.`
			: `${prefix}: qualifying contributions coexist; no single output is selected.`
	);
	lines.push(
		value.finish
			? `${prefix}: ${authority(value.finish)} may declare the scope finished.`
			: `${prefix}: no one may declare it finished; an empty task list is not completion.`
	);
}

const EVIDENCE_WORDS = {
	publication: 'publication',
	review: 'eligible review',
	completion: 'completion',
	selection: 'selection'
} as const;

export const CONTEXTUAL_CHECKS = [
	'Bind referenced role slots to authenticated eligible members; decision-authority roles require exactly one member.',
	'Supply required inputs and verify child rules stay within delegated parent authority.',
	'Verify membership, pinned rule context, exact evidence and distinct reviewer eligibility for each action.',
	'Check authority availability and local execution, filesystem, spending and sharing permissions separately.',
	'This offline inspection does not publish a definition, create a goal, deliver work or launch a process.'
];

/** Explains a normalized blueprint. */
export function explain(value: Blueprint): Explanation {
	const summary = [
		'The goal administrator manages membership and rules separately from work permissions.'
	];
	work(value.work, 'Default rules', summary);
	decisions(value.decisions, 'Default rules', summary);
	for (const name of sortedKeys(value.task_types)) {
		const taskType = value.task_types[name];
		const prefix = `Task type ${rustDebug(name)}`;
		work(taskType.work ?? value.work, prefix, summary);
		decisions(taskType.decisions ?? value.decisions, prefix, summary);
	}
	for (const name of sortedKeys(value.flow)) {
		const stage = value.flow[name];
		summary.push(
			`Stage ${rustDebug(name)}: the goal administrator runs this stage: it creates the configured task and durably delivers ready work to ${selector(stage.recipients)}.`
		);
		const needs =
			stage.requires.length === 0
				? 'no upstream evidence'
				: stage.requires
						.map((item) => `${EVIDENCE_WORDS[item.evidence]} from ${rustDebug(item.stage)}`)
						.join(' and ');
		const uses =
			stage.task_type === null ? 'default rules' : `task type ${rustDebug(stage.task_type)}`;
		summary.push(`Stage ${rustDebug(name)} requires ${needs} and uses ${uses}.`);
	}
	const found = references(value);
	return {
		summary,
		required_roles: found.roles,
		authority_roles: found.authorities,
		required_inputs: Object.keys(value.context.inputs)
			.filter((name) => value.context.inputs[name].required)
			.sort(compareCodePoints),
		contextual_checks: [...CONTEXTUAL_CHECKS]
	};
}
