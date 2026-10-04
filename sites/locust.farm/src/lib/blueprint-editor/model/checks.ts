// Checks the page makes in addition to Locust's rules. They are about what the
// person may not see: hidden characters in text, specific people named in a
// blueprint from someone else, and advice written as instructions to agents.

import { escapePointer } from '../contract/text.ts';
import type { Blueprint, CompletionRule, Selector } from '../contract/types.ts';

export interface PageCheck {
	kind: 'hidden-characters' | 'specific-person' | 'instruction-like-advice';
	/** JSON Pointer of the text or rule. */
	path: string;
	message: string;
	/** Copying waits until this is fixed. */
	blocksCopy: boolean;
}

// Bidirectional controls and tag characters can make text read differently
// from what it says; zero-width characters are only noted, since some scripts
// and emoji need them.
const BLOCKING = /[\u{202a}-\u{202e}\u{2066}-\u{2069}\u{200e}\u{200f}\u{61c}\u{E0000}-\u{E007F}]/u;
const NOTED = /[\u{200b}-\u{200d}\u{2060}\u{feff}]/u;

function hidden(text: string, path: string, what: string, out: PageCheck[]) {
	if (BLOCKING.test(text)) {
		out.push({
			kind: 'hidden-characters',
			path,
			message: `${what} contains invisible characters that can change how it reads. Remove them before copying.`,
			blocksCopy: true
		});
	} else if (NOTED.test(text)) {
		out.push({
			kind: 'hidden-characters',
			path,
			message: `${what} contains zero-width characters. Check that this is intended.`,
			blocksCopy: false
		});
	}
}

const AGENT_INSTRUCTION =
	/\b(ignore|disregard|run|execute|install|curl|sudo|rm -rf)\b|https?:\/\//i;

/** Removes the characters that block copying from a piece of text. */
export function withoutHiddenCharacters(text: string): string {
	return text
		.replace(new RegExp(BLOCKING.source, 'gu'), '')
		.replace(new RegExp(NOTED.source, 'gu'), '');
}

export function pageChecks(blueprint: Blueprint): PageCheck[] {
	const out: PageCheck[] = [];
	for (const [name, role] of Object.entries(blueprint.roles)) {
		const path = `/roles/${escapePointer(name)}`;
		hidden(name, path, `The role name "${name}"`, out);
		hidden(role.description, `${path}/description`, `The description of "${name}"`, out);
	}
	for (const name of Object.keys(blueprint.context.inputs)) {
		hidden(name, `/context/inputs/${escapePointer(name)}`, `The starting material "${name}"`, out);
	}
	for (const name of Object.keys(blueprint.task_types)) {
		hidden(name, `/task_types/${escapePointer(name)}`, `The task type "${name}"`, out);
	}
	for (const name of Object.keys(blueprint.flow)) {
		hidden(name, `/flow/${escapePointer(name)}`, `The stage name "${name}"`, out);
	}
	const guidance = blueprint.context.guidance;
	hidden(guidance, '/context/guidance', 'The advice', out);
	if (AGENT_INSTRUCTION.test(guidance)) {
		out.push({
			kind: 'instruction-like-advice',
			path: '/context/guidance',
			message:
				'The advice reads like instructions to an agent. Advice is shown to agents, but it never gives them permission to do anything.',
			blocksCopy: false
		});
	}
	const person = (path: string) =>
		out.push({
			kind: 'specific-person',
			path,
			message:
				'This blueprint names a specific person by their key. Check that you know who it is, or replace it with a role.',
			blocksCopy: false
		});
	const selector = (value: Selector, path: string) => {
		if (value.kind === 'participant') person(path);
		if (value.kind === 'any')
			value.selectors.forEach((s, i) => selector(s, `${path}/selectors/${i}`));
	};
	const completion = (value: CompletionRule, path: string) => {
		if (value.kind === 'all' || value.kind === 'any') {
			value.rules.forEach((r, i) => completion(r, `${path}/rules/${i}`));
		} else {
			selector(value.by, `${path}/by`);
			if (value.kind === 'check')
				hidden(value.name, `${path}/name`, `The check "${value.name}"`, out);
		}
	};
	const visit = (
		work: Blueprint['work'] | null,
		decisions: Blueprint['decisions'] | null,
		base: string
	) => {
		if (work) {
			selector(work.propose, `${base}/work/propose`);
			selector(work.publish, `${base}/work/publish`);
			work.starts.forEach((start, i) => {
				selector(start.by, `${base}/work/starts/${i}/by`);
				if (start.kind === 'offered') selector(start.to, `${base}/work/starts/${i}/to`);
			});
		}
		if (decisions) {
			completion(decisions.completion, `${base}/decisions/completion`);
			if (decisions.selection?.kind === 'participant') person(`${base}/decisions/selection`);
			if (decisions.finish?.kind === 'participant') person(`${base}/decisions/finish`);
		}
	};
	visit(blueprint.work, blueprint.decisions, '');
	for (const [name, taskType] of Object.entries(blueprint.task_types)) {
		visit(taskType.work, taskType.decisions, `/task_types/${escapePointer(name)}`);
	}
	for (const [name, stage] of Object.entries(blueprint.flow)) {
		const base = `/flow/${escapePointer(name)}`;
		if (stage.runner.kind === 'participant') person(`${base}/runner`);
		selector(stage.recipients, `${base}/recipients`);
	}
	return out;
}
