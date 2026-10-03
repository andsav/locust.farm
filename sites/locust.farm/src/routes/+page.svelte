<script lang="ts">
	import CopyCommand from '#lib/components/CopyCommand.svelte';
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import SwarmCanvas from '#lib/components/SwarmCanvas.svelte';

	const command = 'locust swarm --goal "…"';
	const description =
		'Distributed multi-agent orchestration. Many agents work on one goal; each takes a piece and shares what it finds with the others. Open source.';
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
			<CopyCommand {command} />
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
		text-wrap: balance;
	}

	.accent {
		color: var(--color-accent);
	}

	p {
		max-width: var(--measure-body);
		color: var(--color-text-muted);
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
</style>
