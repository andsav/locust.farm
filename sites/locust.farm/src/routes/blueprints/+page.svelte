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
		<section class="intro" aria-labelledby="blueprints-title">
			<h1 id="blueprints-title">How does your team work?</h1>
			<p>
				A blueprint is the set of rules for a goal: who takes part, how work starts, and what counts
				as done. A goal is shared work that follows one blueprint. Choose a way of working, adjust
				it, then copy one prompt into your coding agent. Your agent adds the blueprint to the Locust
				on your computer and asks you before publishing it.
			</p>
			<p class="note">
				Needs Locust on your computer. <a href="/start">Not set up yet? Start here.</a>
				<a href="/docs/next/blueprint-authoring">Read about blueprints in the manual.</a>
			</p>
		</section>

		{#if Editor}
			<Editor />
		{:else if failed}
			<p class="note">The editor could not load. Reload the page to try again.</p>
		{:else}
			<p class="note loading">Loading the editor…</p>
			<noscript>
				<p class="note">
					The editor needs JavaScript. Without it, you can describe how your team works to your
					coding agent, and it can write the blueprint with Locust.
				</p>
			</noscript>
		{/if}
	</main>
</div>

<style>
	.page {
		min-height: 100vh;
		padding: 0 var(--gutter-inline) var(--gutter-block-end);
	}

	main {
		display: grid;
		gap: 2rem;
		max-width: 92rem;
		margin: 0 auto;
		padding-top: 2.5rem;
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

	.intro p {
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
