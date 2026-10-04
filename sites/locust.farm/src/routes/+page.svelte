<script lang="ts">
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import SwarmCanvas from '#lib/components/SwarmCanvas.svelte';
	import { GUIDE_PATH } from '#lib/onboarding/guide.ts';

	const description = 'Unleash collective intelligence on your hardest problems';
</script>

<svelte:head>
	<title>locust.farm — Distributed Agent Swarm</title>
	<meta name="description" content={description} />
</svelte:head>

<div class="page">
	<SwarmCanvas goalMode="cursor" />
	<SiteHeader />
	<main>
		<h1>Distributed Agent Swarm<span class="accent">.</span></h1>
		<p>{description}</p>
		<div class="actions">
			<a class="start" href={GUIDE_PATH}>Start with one prompt <span aria-hidden="true">→</span></a>
			<p class="status">Early development: nothing to install yet.</p>
		</div>
	</main>
</div>

<style>
	.page {
		position: relative;
		isolation: isolate;
		display: grid;
		/* The column may be narrower than its widest child, which then wraps instead. */
		grid-template-columns: minmax(0, 1fr);
		grid-template-rows: auto 1fr;
		min-height: 100vh;
		/* The small viewport height keeps the call to action above mobile browser toolbars. */
		min-height: 100svh;
		cursor: crosshair;
	}

	main {
		align-self: end;
		display: flex;
		flex-direction: column;
		gap: var(--space-22);
		padding: 0 var(--gutter-inline) var(--gutter-block-end);
	}

	h1 {
		max-width: var(--measure-display);
		font: var(--text-display);
		letter-spacing: var(--tracking-display);
		text-shadow: var(--text-halo);
		text-wrap: balance;
	}

	.accent {
		color: var(--color-accent);
	}

	p {
		max-width: var(--measure-body);
		color: var(--color-text-muted);
		text-shadow: var(--text-halo);
		text-wrap: pretty;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-16);
		padding-top: var(--space-18);
		border-top: var(--border-hairline);
	}

	.start {
		display: inline-flex;
		gap: var(--space-14);
		align-items: center;
		min-height: 2.5rem;
		padding: var(--space-12) var(--space-16);
		border: 1px solid var(--color-accent);
		background: var(--color-bg);
		color: var(--color-text);
		font: var(--text-code);
		cursor: pointer;
		transition-property: background-color, color, scale;
		transition-duration: 150ms;
		transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
	}

	.start:hover {
		background: var(--color-accent);
		color: var(--color-bg);
	}

	.start:active {
		scale: 0.96;
	}

	.status {
		color: var(--color-text-faint);
		font: var(--text-label);
	}

	@media (prefers-reduced-motion: reduce) {
		.start {
			transition: none;
		}

		.start:active {
			scale: none;
		}
	}
</style>
