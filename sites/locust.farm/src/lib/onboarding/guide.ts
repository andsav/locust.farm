/**
 * Content of the first-contact guide at /start. The entry prompt and the routing
 * table follow docs/first-contact.md; change both together.
 */

export const GUIDE_PATH = '/start';

/** The canonical public address the entry prompt points agents at. */
export const GUIDE_URL = `https://locust.farm${GUIDE_PATH}`;

export const ENTRY_PROMPT = `Read ${GUIDE_URL} and follow the instructions for agents. First tell me which harness you are and what you can use. Do not install or change anything until I approve.`;

/**
 * Where Locust's canonical setup lives, once it is published. Until then the guide
 * tells agents to report and stop instead of installing anything.
 */
export const SETUP_ARTIFACT: string | undefined = undefined;

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
			'Locust prepares its configuration. Claude Code 2.1.280 accepted it in a throwaway profile, with no model or account. Not qualified.',
		prerequisite:
			'The Locust daemon and MCP bridge, then a tested run of real tool calls and approvals.'
	},
	{
		name: 'Codex',
		upstream:
			'Stdio MCP in config.toml. Skills are detected automatically; restart if one is missing, and after config changes.',
		locust:
			'Locust prepares its configuration. Codex 0.153.4 read it in a throwaway profile, with no model or account. Not qualified.',
		prerequisite:
			'The Locust daemon and MCP bridge, then a tested run of real tool calls and approvals.'
	},
	{
		name: 'pi',
		upstream:
			'Built-in MCP since v0.99.0, unless an extension owns /mcp, it is turned off, or the session uses the SDK. Run /reload after outside changes.',
		locust: 'Locust prepares a configuration file change. Not qualified.',
		prerequisite: 'The Locust daemon and MCP bridge, then a tested pi version and mode.'
	},
	{
		name: 'Droid',
		upstream:
			'Stdio MCP. MCP config reloads live; a new skill may need a new session. Organization policy can block servers.',
		locust: 'Locust prepares a configuration file change. Not qualified.',
		prerequisite: 'The Locust daemon and MCP bridge, then MCP and skill refresh tested separately.'
	},
	{
		name: 'Any other harness',
		upstream: 'Unknown until you answer the three questions above.',
		locust: 'No generic route yet.',
		prerequisite:
			'A generic route that names the minimum tools, instructions and approvals a harness needs.'
	}
];
