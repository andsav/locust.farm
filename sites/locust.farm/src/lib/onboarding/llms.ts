/**
 * The text of /llms.txt: what the site is, its pages, and the same instructions for
 * agents as the /start guide, as plain Markdown that needs no HTML parsing.
 */

import {
	AGENT_INTRO,
	AGENT_RULE,
	AGENT_STEPS,
	ENTRY_PROMPT,
	GUIDE_URL,
	HARNESS_ROUTES,
	ROUTE_LABELS,
	ROUTING_QUESTIONS,
	SETUP_ARTIFACT
} from './guide.ts';

export const LLMS_PATH = '/llms.txt';

const ORIGIN = new URL(GUIDE_URL).origin;

/** The homepage description, word for word. */
export const SUMMARY = 'Collective artificial intelligence for your toughest problems';

export function llmsText(): string {
	const lastStep = SETUP_ARTIFACT
		? `Follow Locust’s setup for your route: ${SETUP_ARTIFACT}. ${AGENT_STEPS.approve}`
		: AGENT_STEPS.report;

	const status = SETUP_ARTIFACT
		? 'Locust is in early development.'
		: 'Locust is in early development. Setup is not published yet: there is nothing to install, and no harness route is qualified.';

	const questions = ROUTING_QUESTIONS.map(({ name, question }) => `   - ${name}: ${question}`).join(
		'\n'
	);

	const routes = HARNESS_ROUTES.map(
		(route) =>
			`### ${route.name}\n\n` +
			`- ${ROUTE_LABELS.upstream}: ${route.upstream}\n` +
			`- ${ROUTE_LABELS.locust}: ${route.locust}\n` +
			`- ${ROUTE_LABELS.prerequisite}: ${route.prerequisite}`
	).join('\n\n');

	return `# Locust

> ${SUMMARY}

${status}

## Pages

- [Home](${ORIGIN}/): what Locust is for
- [Start](${GUIDE_URL}): the first-contact guide, with the entry prompt for people and the instructions for agents
- [Docs](${ORIGIN}/docs): placeholder; documentation is not written yet

## Entry prompt

A person starts by pasting this into the agent they already use. It carries no command, download, invitation or secret.

> ${ENTRY_PROMPT}

## Instructions for agents

${AGENT_INTRO}

1. ${AGENT_STEPS.identify}
2. ${AGENT_STEPS.answer}
${questions}
3. ${AGENT_STEPS.route}
4. ${lastStep}

${AGENT_RULE}

## Harness routes

${routes}
`;
}
