// Locust's definition rules (crates/locust-core/src/organization/validation.rs),
// ported line for line: the same codes, paths, messages, corrections and order.

import { compareCodePoints, escapePointer, rustDebug, sortedKeys } from './text.ts';
import type {
	Authority,
	Blueprint,
	CompletionRule,
	DecisionRules,
	Diagnostic,
	Selector,
	WorkRules
} from './types.ts';

export interface References {
	roles: string[];
	authorities: string[];
}

const PARTICIPANT_KEY = /^[0-9a-fA-F]{64}$/;
const ONLY_WHITE_SPACE = /^\p{White_Space}*$/u;
const CONTROL = /\p{Cc}/u;

class Validator {
	readonly diagnostics: Diagnostic[] = [];
	readonly roles = new Set<string>();
	readonly authorities = new Set<string>();
	readonly blueprint: Blueprint;

	constructor(blueprint: Blueprint) {
		this.blueprint = blueprint;
	}

	error(code: string, path: string, message: string, correction: string) {
		this.diagnostics.push({
			code,
			severity: 'error',
			phase: 'definition',
			path,
			message,
			correction,
			related_paths: []
		});
	}

	name(name: string, path: string) {
		if (ONLY_WHITE_SPACE.test(name) || CONTROL.test(name)) {
			this.error(
				'invalid_name',
				path,
				'Names must contain visible text and no control characters',
				'Choose a stable descriptive name.'
			);
		}
	}

	role(name: string, path: string) {
		this.roles.add(name);
		if (!Object.hasOwn(this.blueprint.roles, name)) {
			this.error(
				'unknown_role',
				path,
				`Role ${rustDebug(name)} is not declared`,
				'Declare the role in roles or reference an existing role.'
			);
		}
	}

	key(key: string, path: string) {
		if (!PARTICIPANT_KEY.test(key)) {
			this.error(
				'invalid_participant',
				path,
				'Participant identity must be a 32-byte public key encoded as 64 hex characters',
				'Use the authenticated participant public key, or a declared role slot for a reusable template.'
			);
		}
	}

	selector(selector: Selector, path: string, task: boolean, contribution: boolean) {
		switch (selector.kind) {
			case 'role':
				this.role(selector.name, `${path}/name`);
				break;
			case 'participant':
				this.key(selector.key, `${path}/key`);
				break;
			case 'task_creator':
				if (!task) {
					this.error(
						'selector_scope',
						path,
						'There is no task creator in this scope',
						'Use members, a role or a participant in goal-level rules.'
					);
				}
				break;
			case 'contribution_author':
				if (!contribution) {
					this.error(
						'selector_scope',
						path,
						'There is no contribution author in this scope',
						'Use contribution_author only in a contribution completion criterion.'
					);
				}
				break;
			case 'any':
				if (selector.selectors.length === 0) {
					this.error(
						'empty_selector',
						path,
						'An any selector needs at least one alternative',
						'Add an eligible selector, or use nobody to deliberately disable a work action.'
					);
				}
				selector.selectors.forEach((item, index) =>
					this.selector(item, `${path}/selectors/${index}`, task, contribution)
				);
				break;
		}
	}

	authority(authority: Authority, path: string) {
		if (authority.kind === 'role') {
			this.role(authority.name, `${path}/name`);
			this.authorities.add(authority.name);
		} else {
			this.key(authority.key, `${path}/key`);
		}
	}

	work(work: WorkRules, path: string) {
		this.selector(work.propose, `${path}/propose`, false, false);
		this.selector(work.publish, `${path}/publish`, false, false);
		work.starts.forEach((start, index) => {
			const startPath = `${path}/starts/${index}`;
			this.selector(start.by, `${startPath}/by`, true, false);
			if (start.kind === 'offered') this.selector(start.to, `${startPath}/to`, true, false);
		});
	}

	completion(rule: CompletionRule, path: string) {
		if (rule.kind === 'all' || rule.kind === 'any') {
			if (rule.rules.length === 0) {
				this.error(
					'empty_criteria',
					path,
					'Completion groups need at least one criterion',
					'Add an explicit completion criterion.'
				);
			}
			rule.rules.forEach((item, index) => this.completion(item, `${path}/rules/${index}`));
			return;
		}
		this.selector(rule.by, `${path}/by`, true, true);
		const fixed = fixedMembers(rule.by);
		if (fixed !== null && fixed.size === 0) {
			this.error(
				'impossible_completion',
				`${path}/by`,
				'This criterion has no possible eligible signer',
				'Choose an eligible participant selector.'
			);
		}
		if (rule.kind === 'check') this.name(rule.name, `${path}/name`);
		if (rule.kind === 'reviews') {
			if (rule.count === 0) {
				this.error(
					'invalid_threshold',
					`${path}/count`,
					'Review thresholds must be positive',
					'Require one or more distinct eligible reviewers.'
				);
			}
			if (fixed !== null) {
				if (rule.exclude_author) fixed.delete('contribution_author');
				if (rule.count > fixed.size) {
					this.error(
						'impossible_threshold',
						`${path}/count`,
						'The requested distinct-reviewer threshold exceeds the explicitly possible identities',
						'Add eligible identities, reduce the threshold, or use a role whose membership is checked when binding the template.'
					);
				}
			}
		}
	}

	decisions(decisions: DecisionRules, path: string) {
		this.completion(decisions.completion, `${path}/completion`);
		if (decisions.selection) this.authority(decisions.selection, `${path}/selection`);
		if (decisions.finish) this.authority(decisions.finish, `${path}/finish`);
	}

	run() {
		const value = this.blueprint;
		for (const name of sortedKeys(value.roles)) this.name(name, `/roles/${escapePointer(name)}`);
		for (const name of sortedKeys(value.context.inputs)) {
			this.name(name, `/context/inputs/${escapePointer(name)}`);
		}
		this.work(value.work, '/work');
		this.decisions(value.decisions, '/decisions');
		for (const name of sortedKeys(value.task_types)) {
			const taskType = value.task_types[name];
			const path = `/task_types/${escapePointer(name)}`;
			this.name(name, path);
			if (taskType.work) this.work(taskType.work, `${path}/work`);
			if (taskType.decisions) this.decisions(taskType.decisions, `${path}/decisions`);
		}
		const dependencies = new Map<string, Set<string>>();
		for (const name of sortedKeys(value.flow)) {
			const stage = value.flow[name];
			const path = `/flow/${escapePointer(name)}`;
			this.name(name, path);
			this.selector(stage.recipients, `${path}/recipients`, false, false);
			if (stage.task_type !== null && !Object.hasOwn(value.task_types, stage.task_type)) {
				this.error(
					'unknown_task_type',
					`${path}/task_type`,
					`Task type ${rustDebug(stage.task_type)} is not declared`,
					'Declare the task type or remove the reference to inherit the default rules.'
				);
			}
			const required = new Set<string>();
			stage.requires.forEach((requirement, index) => {
				const requirementPath = `${path}/requires/${index}`;
				const upstream = Object.hasOwn(value.flow, requirement.stage)
					? value.flow[requirement.stage]
					: undefined;
				if (upstream) {
					required.add(requirement.stage);
					const upstreamType =
						upstream.task_type !== null && Object.hasOwn(value.task_types, upstream.task_type)
							? value.task_types[upstream.task_type]
							: undefined;
					const decisions = upstreamType?.decisions ?? value.decisions;
					if (requirement.evidence === 'selection' && decisions.selection === null) {
						this.error(
							'unavailable_evidence',
							`${requirementPath}/evidence`,
							'The upstream stage has no selection authority',
							'Require completion/publication/review evidence, or explicitly configure upstream selection.'
						);
					}
				} else {
					this.error(
						'unknown_stage',
						`${requirementPath}/stage`,
						`Stage ${rustDebug(requirement.stage)} is not declared`,
						'Reference a declared stage.'
					);
				}
			});
			dependencies.set(name, required);
		}
		while (dependencies.size > 0) {
			const ready = [...dependencies].filter(([, needs]) => needs.size === 0).map(([n]) => n);
			if (ready.length === 0) {
				const names = [...dependencies.keys()].sort(compareCodePoints).join(', ');
				this.error(
					'flow_cycle',
					'/flow',
					`Flow has a dependency cycle involving: ${names}`,
					'Remove a cyclic prerequisite; retries and new rounds are explicit runtime actions.'
				);
				break;
			}
			for (const name of ready) dependencies.delete(name);
			for (const needs of dependencies.values()) for (const name of ready) needs.delete(name);
		}
	}
}

/** The identities a selector can name for certain, or null when it depends on membership. */
function fixedMembers(selector: Selector): Set<string> | null {
	switch (selector.kind) {
		case 'nobody':
			return new Set();
		case 'participant':
			return new Set([selector.key.toLowerCase()]);
		case 'task_creator':
			return new Set(['task_creator']);
		case 'contribution_author':
			return new Set(['contribution_author']);
		case 'any': {
			const members = new Set<string>();
			for (const item of selector.selectors) {
				const fixed = fixedMembers(item);
				if (fixed === null) return null;
				for (const member of fixed) members.add(member);
			}
			return members;
		}
		default:
			return null;
	}
}

export function validate(blueprint: Blueprint): Diagnostic[] {
	const validator = new Validator(blueprint);
	validator.run();
	return validator.diagnostics;
}

export function references(blueprint: Blueprint): References {
	const validator = new Validator(blueprint);
	validator.run();
	return {
		roles: [...validator.roles].sort(compareCodePoints),
		authorities: [...validator.authorities].sort(compareCodePoints)
	};
}
