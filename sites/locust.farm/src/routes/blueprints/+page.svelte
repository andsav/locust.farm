<script lang="ts">
	import { onMount, type Component } from 'svelte';
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import { BLUEPRINTS_PATH } from '#lib/site.ts';

	const description =
		'Choose how the people and agents in a goal work together, then copy one prompt that adds the blueprint to the Locust on your computer.';

	let Editor = $state<Component | null>(null);
	let failed = $state(false);

	onMount(async () => {
		try {
			Editor = (await import('#lib/blueprint-editor/ui/BlueprintEditor.svelte')).default;
		} catch {
			failed = true;
		}
	});
</script>

<svelte:head>
	<title>Blueprints — locust.farm</title>
	<meta name="description" content={description} />
	<link rel="canonical" href={`https://locust.farm${BLUEPRINTS_PATH}`} />
</svelte:head>

<div class="page">
	<SiteHeader />
	<main>
		<h1 id="blueprints-title">How does your team work?</h1>
		<p class="lede">
			A blueprint is the rules for one shared goal. Pick a way of working, change any of its four
			points, then copy the prompt.
		</p>

		{#if Editor}
			<Editor />
		{:else if failed}
			<p class="note">The editor could not load. Reload the page to try again.</p>
		{:else}
			<p class="note loading">Loading the editor…</p>
			<noscript>
				<div class="intro">
					<p>
						A blueprint is the set of rules for a goal: who adds tasks, who works on them, when a
						result counts and whether one result is picked. This page lets you choose a way of
						working, adjust it, and copy one prompt that adds it to the Locust on your computer.
					</p>
					<p>
						The editor needs JavaScript. Without it, you can describe how your team works to your
						coding agent, and it can write the blueprint with Locust.
					</p>
					<p class="note">
						<a href="/start">Not set up yet? Start here.</a>
						<a href="/docs/next/blueprint-authoring">Read about blueprints in the manual.</a>
					</p>
				</div>
			</noscript>
		{/if}
	</main>
</div>

<style>
	.page {
		display: flex;
		flex-direction: column;
		min-height: 100dvh;
		padding: 0 var(--gutter-inline) var(--gutter-block-end);
	}

	main {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 1.25rem;
		width: 100%;
		max-width: 110rem;
		margin: 0 auto;
		padding-top: 1.5rem;
	}

	.intro {
		display: grid;
		gap: 0.75rem;
		max-width: 46rem;
	}

	h1 {
		font: var(--text-display);
		letter-spacing: var(--tracking-display);
		text-wrap: balance;
	}

	.lede {
		max-width: 46rem;
		margin: -0.5rem 0 0;
		color: var(--color-text-muted);
		text-wrap: pretty;
	}

	.intro p {
		margin-bottom: 0.75rem;
		color: var(--color-text-muted);
		text-wrap: pretty;
	}

	.note {
		color: var(--color-text-subtle);
	}

	.note a {
		margin-right: 0.75rem;
		text-decoration: underline;
	}
</style>
