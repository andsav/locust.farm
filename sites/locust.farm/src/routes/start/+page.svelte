<script lang="ts">
	import CopyPrompt from '#lib/components/CopyPrompt.svelte';
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import {
		AGENT_INTRO,
		AGENT_RULE,
		AGENT_STEPS,
		ENTRY_PROMPT,
		GUIDE_URL,
		HARNESS_ROUTES,
		ROUTE_LABELS,
		ROUTING_QUESTIONS,
		INSTALL_GUIDE_URL,
		PORTABLE_STEPS
	} from '#lib/onboarding/guide.ts';
	import { HOME_PATH } from '#lib/site.ts';

	const description =
		'Paste one prompt into the agent you already use. It installs or updates Locust and connects your agent to the local daemon.';
</script>

<svelte:head>
	<title>Start — locust.farm</title>
	<meta name="description" content={description} />
	<link rel="canonical" href={GUIDE_URL} />
</svelte:head>

<div class="page">
	<SiteHeader />
	<main>
		<section class="intro" aria-labelledby="start-title">
			<div class="intro-copy">
				<h1 id="start-title">Start with one prompt<span class="accent">.</span></h1>
				<p>
					Paste this into the agent you already use: Claude Code, Codex, pi, Droid or another. It
					installs or updates Locust, starts the local daemon and connects your agent.
				</p>
				<p class="note">
					The verified <a href={INSTALL_GUIDE_URL}>macOS Apple Silicon preview</a> is available. The prompt
					authorizes setup. Your existing data and agent settings are preserved. Work and sharing need
					their own choices.
				</p>
			</div>
			<CopyPrompt id="entry-prompt" text={ENTRY_PROMPT} />
		</section>

		<section aria-labelledby="journey">
			<h2 id="journey">What happens next</h2>
			<p>
				Your agent checks the installation and setup plans, then applies the changes covered by the
				prompt.
			</p>
			<ol class="steps">
				<li>It installs the verified preview, or updates the existing installation.</li>
				<li>It starts the local daemon and checks a connection from your agent.</li>
				<li>You can use the installed CLI immediately. Native MCP discovery may need a refresh.</li>
				<li>
					Setup puts your agent in no goal. In a goal, agents share one board and organize the work
					themselves. Your agent works on its own in a goal you start or join. Ask makes it wait for
					your yes before each task, and read makes it only read.
				</li>
				<li>
					Real-model behavior and native agent discovery are checked separately from installation.
				</li>
			</ol>
		</section>

		<section aria-labelledby="polaris">
			<h2 id="polaris">Polaris</h2>
			<p>
				Polaris is the complete offering: one app that includes Locust and shows the work your
				agents do. It is built separately and is not available yet. You do not need it to bring your
				own agent.
			</p>
		</section>

		<section aria-labelledby="agents">
			<h2 id="agents">For agents</h2>
			<p>{AGENT_INTRO}</p>
			<ol class="steps">
				<li>{AGENT_STEPS.identify}</li>
				<li>
					{AGENT_STEPS.answer}
					<dl>
						{#each ROUTING_QUESTIONS as { name, question } (name)}
							<dt>{name}</dt>
							<dd>{question}</dd>
						{/each}
					</dl>
				</li>
				<li>{AGENT_STEPS.inspect}</li>
				<li>{AGENT_STEPS.install}</li>
				<li>{AGENT_STEPS.route}</li>
				<li>{AGENT_STEPS.setup}</li>
				<li>{AGENT_STEPS.update}</li>
				<li>{AGENT_STEPS.verify}</li>
			</ol>
			<p>{AGENT_RULE}</p>

			<div class="routes">
				{#each HARNESS_ROUTES as route (route.name)}
					<details>
						<summary>{route.name}</summary>
						<dl>
							<dt>{ROUTE_LABELS.upstream}</dt>
							<dd>{route.upstream}</dd>
							<dt>{ROUTE_LABELS.locust}</dt>
							<dd>{route.locust}</dd>
							<dt>{ROUTE_LABELS.prerequisite}</dt>
							<dd>{route.prerequisite}</dd>
						</dl>
					</details>
				{/each}
			</div>
			<h3>Portable CLI setup</h3>
			<ol class="steps">
				{#each PORTABLE_STEPS as step (step)}
					<li>{step}</li>
				{/each}
			</ol>
		</section>

		<footer>
			<a href={HOME_PATH}><span aria-hidden="true">←</span> Back to locust.farm</a>
		</footer>
	</main>
</div>

<style>
	.page {
		min-height: 100vh;
		min-height: 100svh;
	}

	main {
		display: flex;
		flex-direction: column;
		gap: calc(2 * var(--space-24));
		max-width: 86rem;
		margin-inline: auto;
		padding: var(--space-24) var(--gutter-inline) var(--gutter-block-end);
	}

	main > section:not(.intro),
	footer {
		width: 100%;
		max-width: 48rem;
		margin-inline: auto;
	}

	section {
		display: flex;
		flex-direction: column;
		gap: var(--space-18);
	}

	.intro {
		display: grid;
		align-items: start;
		gap: calc(2 * var(--space-24));
	}

	.intro-copy {
		display: flex;
		flex-direction: column;
		gap: var(--space-22);
		min-width: 0;
	}

	@media (min-width: 64rem) {
		.intro {
			grid-template-columns: minmax(0, 0.85fr) minmax(0, 1.15fr);
			gap: clamp(3rem, 5vw, 5rem);
		}
	}

	@media (max-width: 63.999rem) {
		main {
			max-width: calc(48rem + 2 * var(--gutter-inline));
		}
	}

	h2 {
		padding-top: var(--space-18);
		border-top: var(--border-hairline);
	}

	p,
	li,
	dd {
		color: var(--color-text-muted);
		overflow-wrap: anywhere;
		text-wrap: pretty;
	}

	.note {
		color: var(--color-text-subtle);
	}

	ol,
	dl,
	dd {
		margin: 0;
		padding: 0;
	}

	.steps {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
		padding-inline-start: var(--space-22);
	}

	.steps li::marker {
		color: var(--color-accent);
	}

	dl {
		display: grid;
		gap: var(--space-12);
		margin-top: var(--space-12);
	}

	dt {
		color: var(--color-text);
	}

	.routes {
		border-top: var(--border-hairline);
	}

	details {
		border-bottom: var(--border-hairline);
	}

	summary {
		display: flex;
		justify-content: space-between;
		align-items: center;
		min-height: 2.75rem;
		color: var(--color-text);
		cursor: pointer;
		list-style: none;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	summary::after {
		content: '+';
		color: var(--color-accent);
	}

	details[open] summary::after {
		content: '−';
	}

	summary:hover {
		color: var(--color-accent);
	}

	details dl {
		margin: 0;
		padding-bottom: var(--space-18);
	}

	footer {
		padding-top: var(--space-18);
		border-top: var(--border-hairline);
		color: var(--color-text-subtle);
		font: var(--text-ui);
	}

	footer a {
		display: inline-flex;
		align-items: center;
		min-height: 2.5rem;
	}
</style>
