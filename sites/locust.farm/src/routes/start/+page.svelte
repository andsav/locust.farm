<script lang="ts">
	import CopyPrompt from '#lib/components/CopyPrompt.svelte';
	import SiteHeader from '#lib/components/SiteHeader.svelte';
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
	} from '#lib/onboarding/guide.ts';
	import { HOME_PATH } from '#lib/site.ts';

	const description =
		'Paste one prompt into the agent you already use. It reads this guide, tells you what it can do, and asks before it changes anything.';
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
			<h1 id="start-title">Start with one prompt<span class="accent">.</span></h1>
			<p>
				Paste this into the agent you already use: Claude Code, Codex, pi, Droid or another. It
				reads this guide, tells you what it can do, and asks before it changes anything.
			</p>
			<CopyPrompt id="entry-prompt" text={ENTRY_PROMPT} />
			<p class="note">
				Locust is in early development. Setup is not published yet, so today your agent reports what
				it found and stops. Nothing is installed. Availability reviewed {AVAILABILITY.reviewed};
				<a href="/docs/next/status">read the qualification boundaries</a>.
			</p>
		</section>

		<section aria-labelledby="journey">
			<h2 id="journey">Planned collaboration journey</h2>
			<p>
				This describes the intended public journey. It is not a published installation procedure.
			</p>
			<ol class="steps">
				<li>Your agent says which harness it is and what it can use.</li>
				<li>It lists every change setup would make. You approve or decline.</li>
				<li>A second agent on your machine joins the same task. You decide what it may do.</li>
				<li>
					Later, you invite someone. You choose who and exactly what to share. They approve on their
					side.
				</li>
				<li>
					Results come back for review. Accepting one does not change your files; you apply it when
					you choose.
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
				<li>{AGENT_STEPS.route}</li>
				<li>
					{#if SETUP_ARTIFACT}
						Follow <a href={SETUP_ARTIFACT}>Locust’s setup</a> for your route. {AGENT_STEPS.approve}
					{:else}
						{AGENT_STEPS.report}
					{/if}
				</li>
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
		</section>

		<footer>
			<a href={HOME_PATH}><span aria-hidden="true">←</span> back to locust.farm</a>
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
		max-width: calc(var(--measure-body) + 2 * var(--gutter-inline));
		padding: var(--space-24) var(--gutter-inline) var(--gutter-block-end);
	}

	section {
		display: flex;
		flex-direction: column;
		gap: var(--space-18);
	}

	.intro {
		gap: var(--space-22);
	}

	h1 {
		font: var(--text-display);
		letter-spacing: var(--tracking-display);
		text-wrap: balance;
	}

	.accent {
		color: var(--color-accent);
	}

	h2 {
		margin: 0;
		padding-top: var(--space-18);
		border-top: var(--border-hairline);
		color: var(--color-text-subtle);
		font: var(--text-label);
		letter-spacing: var(--tracking-label);
		text-transform: uppercase;
	}

	p,
	li,
	dd {
		color: var(--color-text-muted);
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
		font: var(--text-label);
		letter-spacing: var(--tracking-label);
		text-transform: uppercase;
	}

	footer a {
		display: inline-flex;
		align-items: center;
		min-height: 2.5rem;
	}
</style>
