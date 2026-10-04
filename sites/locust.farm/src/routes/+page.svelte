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
			<a class="button primary" href={GUIDE_PATH}
				>Start with one prompt <span aria-hidden="true">→</span></a
			>
			<a
				class="button quiet"
				href="https://github.com/andsav/locust.farm"
				target="_blank"
				rel="noopener noreferrer">GitHub <span aria-hidden="true">↗</span></a
			>
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
		text-shadow: var(--text-halo);
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

	/* The GitHub link has no surface of its own, so its text takes the halo. */
	.quiet {
		text-shadow: var(--text-halo);
	}
</style>
