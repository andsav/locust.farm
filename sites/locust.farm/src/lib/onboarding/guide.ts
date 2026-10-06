import availability from '../../../../../docs/reference/availability.json' with { type: 'json' };

/** Shared by /start, /llms.txt and the copy button; see docs/first-contact.md. */
export const GUIDE_PATH = '/start';
export const GUIDE_URL = `https://locust.farm${GUIDE_PATH}`;
export const INSTALL_GUIDE_URL = 'https://locust.farm/downloads/install.md';
export const INSTALLER_URL = 'https://locust.farm/downloads/install.sh';
export const RELEASE_METADATA_URL = 'https://locust.farm/downloads/latest.json';
export const AVAILABILITY = availability;

/** Self-contained: the agent does not need access to the authenticated website. */
export const ENTRY_PROMPT = `Install or update Locust on this machine and connect the agent I am using now. Use the official public instructions at ${INSTALL_GUIDE_URL} and the verified installer at ${INSTALLER_URL}; you do not need access to /start. Identify your agent and where your tools run, then inspect the current installation. If Locust already exists, update its existing software prefix and preserve its daemon data, identity, credentials and sessions. Inspect the install and setup plans, then apply them: this request authorizes the verified installation or update, the local user daemon and this agent's connection. Run locust --owner up --client CLIENT --plan, show me the plan, then run it again with the --name and --confirm it printed; use the installed CLI if MCP needs a refresh. Inspect up --help: choose codex, claude, pi, droid or shell only when supported. Otherwise finish daemon setup with service plan/apply/start, enroll a dedicated CLI agent, create its protected session and use that scoped CLI connection. Do not stop just because a route lacks end-to-end qualification. Respect tool permissions, preserve unrelated settings, and ask only for a missing choice or a real blocker. Do not start or join goals, set levels, allow tasks or connect folders; those are mine to decide. Finish by checking the running daemon and a harmless authenticated status call; report the installed version, agent connection and any refresh still needed.`;

export const ROUTING_QUESTIONS = [
	{
		name: 'Execution host',
		question:
			'Do your tools run on the person’s computer, a remote machine or a container? Name the OS and architecture. A remote install does not install on their computer.'
	},
	{
		name: 'Transport',
		question:
			'Can this session run local shell commands or reach a local stdio MCP server under its current policy? CLI access can complete setup without waiting for native MCP discovery.'
	},
	{
		name: 'Instructions',
		question:
			'Can you load the installed SKILL.md, or read it directly as a file? Report whether native discovery needs a refresh or a new chat.'
	}
] as const;

export interface HarnessRoute {
	name: string;
	upstream: string;
	locust: string;
	prerequisite: string;
}

export const HARNESS_ROUTES: readonly HarnessRoute[] = [
	{
		name: 'Claude Code',
		upstream: 'Stdio MCP and local skills. Refresh or start a new chat if discovery is stale.',
		locust: 'Select --client claude. Setup owns one MCP entry, skill and bound CLI launcher.',
		prerequisite:
			'An existing workspace and the active profile home; defaults use the current directory and HOME.'
	},
	{
		name: 'Codex',
		upstream: 'Stdio MCP and local skills. A configuration change may require a new chat.',
		locust: 'Select --client codex. Setup owns one MCP entry, skill and bound CLI launcher.',
		prerequisite:
			'An existing workspace and the active profile home; inspect CODEX_HOME before selecting the standard layout.'
	},
	{
		name: 'pi',
		upstream:
			'Local CLI and skills; native MCP availability depends on the installed version and mode.',
		locust:
			'Select --client pi. Read the installed skill and use its bound CLI while native MCP refresh is pending.',
		prerequisite:
			'An existing workspace and the active profile home; inspect PI_CODING_AGENT_DIR before selecting the standard layout.'
	},
	{
		name: 'Droid',
		upstream:
			'Stdio MCP in .factory/mcp.json reloads live. Skills live under .factory/skills; a new chat may be needed.',
		locust:
			'Select --client droid when the installed up command supports it. Setup preserves unrelated Factory configuration and provides a bound CLI launcher.',
		prerequisite:
			'Inspect locust up --help: earlier published binaries may require the portable CLI route below until a release includes Droid onboarding.'
	},
	{
		name: 'Any other harness',
		upstream:
			'Local shell commands and the ability to read a file are enough for the portable CLI route.',
		locust:
			'Select --client shell when supported. It writes portable instructions, a bound CLI and an MCP connection descriptor under .local/share/locust-agent in the selected profile; it does not edit another app’s configuration.',
		prerequisite:
			'If the published binary lacks shell onboarding, install/start its user service and enroll a scoped CLI principal using the installed manual. No native adapter is required to install the daemon.'
	}
];

export const AGENT_INTRO =
	'The entry prompt authorizes local setup. Inspect the plans and continue within that scope:';

export const AGENT_STEPS = {
	identify:
		'Briefly identify the agent application, its version if known, and the execution host. Unknown version or refresh behavior is not itself an installation blocker.',
	answer: 'Check these capabilities without turning them into a separate approval round:',
	inspect:
		'Inspect the OS/architecture, locust on PATH, the default software prefix, existing onboarding/service records and the current workspace. Read records without printing secret files. Reuse an existing owned prefix, daemon home and profile binding. Do not silently select a new identity or daemon because setup already exists.',
	install: `Read ${INSTALL_GUIDE_URL} and ${RELEASE_METADATA_URL}; if your web reader cannot fetch plain text, use curl through the permitted shell. Only macOS Apple Silicon is currently published. Download ${INSTALLER_URL} to a temporary file, inspect it and run sh INSTALLER --plan with the selected --prefix and --bin-dir. Inspect the verified plan, then run the same installer without --plan. This also updates an existing owned installation; use the absolute installed current/locust path without changing shell startup files. Preserve a foreign executable or unowned service and report the collision.`,
	route:
		'Inspect the installed executable’s up --help and choose the matching route below. Follow commands supported by those bytes, rather than assuming the development source has already been released. Missing qualification is evidence to report, not a command to stop.',
	setup:
		'For a supported client, run CURRENT/locust --home DATA_HOME --owner up --client CLIENT --profile-home PROFILE_HOME --workspace WORKSPACE --plan. Review it, then repeat the same selections with the --name NAME and --confirm PLAN_ID it printed instead of --plan. Setup authorization includes the user service, enrollment, protected session, instructions, launcher and scoped MCP entry; it puts the agent in no goal. Read the installed skill and use its bound locust-cli now when shell access is permitted. If the binary lacks this client, use the portable CLI procedure below to finish daemon setup without changing another client’s profile.',
	update:
		'Software activation does not replace a running daemon. For an update, use service status to inspect the owned service and service start with its original prefix, kind, profile home, daemon home and log directory to restart it when the running code differs. Recheck API readiness. Preserve incompatible existing state and report it for a fresh-state choice; never erase or migrate it or kill an unrelated/foreground process.',
	verify:
		'Check install status, service state and authenticated daemon API readiness. Run the installed bound locust-cli status, or the harmless locust_status MCP tool if already available. Report CLI and native MCP discovery separately. If a refresh is needed, give its exact next step; successful CLI access is usable now. Preserve the journal on interruption and retry the same selections. Finish with version/source, installed paths, daemon result and agent connection; name unverified boundaries.'
} as const;

/** Works with the existing public CLI even before new up adapters are released. */
export const PORTABLE_STEPS = [
	'Use CURRENT/locust --json service plan --prefix PREFIX --kind launchd --profile-home HOME --daemon-home DATA_HOME --log-dir DATA_HOME/logs. Inspect the plan; apply those same selections with service apply --expect-plan PLAN_SHA256, then service start. Run CURRENT/locust --home DATA_HOME --owner doctor to verify the daemon. On a supported locally supplied Linux candidate, select systemd instead.',
	'Inspect owner status and existing named credentials first. Reuse a dedicated agent for this client, or enroll a uniquely named one with CURRENT/locust --home DATA_HOME --owner agent enroll NAME. The command returns its protected credential file path; keep secret bytes out of the conversation. Create/reuse its fixed session with CURRENT/locust session create ABSOLUTE_SESSION_PATH. Retain the name and those paths for retries.',
	'Use CURRENT/locust --home DATA_HOME --credential CREDENTIAL_PATH --session SESSION_PATH status for a harmless agent-authenticated check. Read the installed current/skills/locust/SKILL.md and use that same scoped command prefix for CLI operations. Native MCP registration can be configured separately through the agent’s documented interface using locust mcp and those protected-file paths; no policy change is authorized.'
] as const;

export const AGENT_RULE =
	'Keep client approvals, sandbox rules and organization policy intact. Use CLI access only when that access is allowed; never route around a denied operation. Ask for a decision when tools are unavailable, a required workspace/profile choice is ambiguous, existing state is incompatible or a change exceeds the setup request. Setup never puts the agent in a goal, sets a level or connects a folder.';

export const ROUTE_LABELS = {
	upstream: 'Connection',
	locust: 'Setup',
	prerequisite: 'Check first'
} as const;
