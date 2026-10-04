/**
 * The text of /llms.txt: what the site is, its pages, and the same instructions for
 * agents as the /start guide, as plain Markdown that needs no HTML parsing.
 */

import {
	AVAILABILITY,
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
export const SUMMARY = 'Unleash collective intelligence on your hardest problems';

export function llmsText(): string {
	const lastStep = SETUP_ARTIFACT
		? `Follow Locust’s setup for your route: ${SETUP_ARTIFACT}. ${AGENT_STEPS.approve}`
		: AGENT_STEPS.report;

	const status = SETUP_ARTIFACT
		? 'Locust is in early development.'
		: `Locust is in early development. A macOS Apple Silicon terminal preview is published: ${AVAILABILITY.publication.installationGuideUrl}. This first-contact harness route remains unqualified.`;

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

Availability reviewed ${AVAILABILITY.reviewed}; organization runtime: ${AVAILABILITY.organizationRuntime}.

## Pages

- [Home](${ORIGIN}/): what Locust is for
- [Start](${GUIDE_URL}): the first-contact guide, with the entry prompt for people and the instructions for agents
- [Formations](${ORIGIN}/formations): an editor for formations, the rules a goal follows. It needs Locust on the person's computer and copies a prompt that follows docs/formation-prompt.md: the agent checks the formation with Locust, saves a private draft and asks before publishing
- [Docs](${ORIGIN}/docs): unreleased development documentation
- [Development inventory](${ORIGIN}/docs/next/index.json): source commit, contract versions, page status and raw Markdown URLs
- [Formation authoring](${ORIGIN}/docs/next/formation-authoring): offline checks, private drafts/publication and explicit runtime bindings
- [Runtime contract](${ORIGIN}/docs/next/reference/runtime.contract.json): generated CLI, API, MCP and signed-event definitions
- [Formation schema](${ORIGIN}/docs/next/reference/organization.schema.json): current exported JSON shape
- [Authoring contract](${ORIGIN}/docs/next/reference/organization.contract.json): exact offline operations, examples and capability boundaries
- [Peer-review example](${ORIGIN}/docs/next/examples/peer-review.json): exact checked definition; not a running organization
- [Availability](${ORIGIN}/docs/next/reference/availability.json): reviewed publication and qualification facts

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
