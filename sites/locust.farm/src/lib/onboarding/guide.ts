import availability from '../../../../../docs/reference/availability.json' with { type: 'json' };

/**
 * Content of the first-contact guide at /start. The entry prompt and the routing
 * table follow docs/first-contact.md; change both together.
 */

export const GUIDE_PATH = '/start';

/** The canonical public address the entry prompt points agents at. */
export const GUIDE_URL = `https://locust.farm${GUIDE_PATH}`;

export const ENTRY_PROMPT = `Read ${GUIDE_URL} and follow the instructions for agents. First tell me which harness you are and what you can use. Do not install or change anything until I approve.`;

/**
 * The qualified first-contact harness setup, once published. The terminal-only
 * preview is listed separately and does not qualify this route.
 */
export const AVAILABILITY = availability;
export const SETUP_ARTIFACT: string | undefined =
	availability.publication.installerUrl ?? undefined;

/** The questions an agent answers about its own session before choosing a route. */
export const ROUTING_QUESTIONS = [
	{
		name: 'Transport',
		question:
			'Can this session reach a local stdio MCP server, or run a command line that can reach a local socket under its current policy?'
	},
	{
		name: 'Instructions',
		question: 'Can it load a SKILL.md skill, and what refresh does a new or changed skill need?'
	},
	{
		name: 'Approvals',
		question:
			'Can it ask the person before a change, and does a policy block MCP servers, local commands or file writes?'
	}
] as const;

export interface HarnessRoute {
	name: string;
	/** What the client itself documents. This is not Locust support. */
	upstream: string;
	/** What Locust has actually tested with this client. */
	locust: string;
	/** What has to exist before this route can be followed. */
	prerequisite: string;
}

export const HARNESS_ROUTES: readonly HarnessRoute[] = [
	{
		name: 'Claude Code',
		upstream:
			'Stdio MCP. Skills reload live in watched directories; a new top-level skills directory needs /reload-skills.',
		locust:
			'Locust implements installed onboarding and authenticated daemon readiness. Disposable macOS onboarding checks used Claude profiles. These earlier checks do not qualify the API 2/protocol 2 replacement or a native real-model tool roundtrip.',
		prerequisite:
			'A published verified candidate and qualification of real native tool calls and approvals.'
	},
	{
		name: 'Codex',
		upstream:
			'Stdio MCP in config.toml. Skills are detected automatically; restart if one is missing, and after config changes.',
		locust:
			'Locust implements installed onboarding and authenticated daemon readiness. Disposable macOS onboarding checks used Codex profiles. These earlier checks do not qualify the API 2/protocol 2 replacement or a native real-model tool roundtrip.',
		prerequisite:
			'A published verified candidate and qualification of real native tool calls and approvals.'
	},
	{
		name: 'pi',
		upstream:
			'Built-in MCP since v0.99.0, unless an extension owns /mcp, it is turned off, or the session uses the SDK. Run /reload after outside changes.',
		locust: 'Locust prepares a configuration file change. Not qualified.',
		prerequisite: 'A published verified candidate, then a tested pi version and mode.'
	},
	{
		name: 'Droid',
		upstream:
			'Stdio MCP. MCP config reloads live; a new skill may need a new session. Organization policy can block servers.',
		locust: 'Locust prepares a configuration file change. Not qualified.',
		prerequisite: 'A published verified candidate, then MCP and skill refresh tested separately.'
	},
	{
		name: 'Any other harness',
		upstream: 'Unknown until you answer the three questions above.',
		locust: 'No generic route yet.',
		prerequisite:
			'A generic route that names the minimum tools, instructions and approvals a harness needs.'
	}
];

/**
 * The instructions for agents, shared by the /start page and /llms.txt. The steps
 * are followed in order; the standing rule applies throughout and sits outside
 * them, so it is not read as something to do after stopping.
 */
export const AGENT_INTRO = 'A person pasted the entry prompt and sent you here. Do this, in order:';

export const AGENT_STEPS = {
	identify:
		'Tell the person your harness, its version and its mode. Name the harness, not the model. Say “unknown” when you cannot tell.',
	answer: 'Answer three questions about this session:',
	route: 'Find your harness below. If it is not listed, use “Any other harness”.',
	/** The last step while no setup is published. */
	report:
		'The terminal preview is published, but this first-contact route is not qualified. Report your harness, your answers and your route’s prerequisite, then stop. Do not install, download or run anything for Locust.',
	/** The last step once SETUP_ARTIFACT is set; it follows the link to the setup. */
	approve: 'Show every change it would make and wait for the person to approve.'
} as const;

export const AGENT_RULE =
	'At every step: never edit stored approvals, weaken a policy or work around a block. Tell the person what was refused and let them decide.';

/** Labels for the three fields of a harness route. */
export const ROUTE_LABELS = {
	upstream: 'The client documents',
	locust: 'Locust today',
	prerequisite: 'Needed first'
} as const;
