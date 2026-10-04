<script lang="ts">
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import { HOME_PATH } from '#lib/site.ts';

	const description = 'Documentation for Locust is coming soon.';
</script>

<svelte:head>
	<title>Docs — locust.farm</title>
	<meta name="description" content={description} />
</svelte:head>

<div class="page">
	<SiteHeader />
	<main>
		<h1>Documentation<span class="accent">.</span></h1>
		<p>
			This is where Locust’s documentation will live. We’re still putting it together, so check back
			soon.
		</p>
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
		gap: var(--space-22);
		max-width: calc(var(--measure-body) + 2 * var(--gutter-inline));
		padding: var(--space-24) var(--gutter-inline) var(--gutter-block-end);
	}

	h1 {
		font: var(--text-display);
		letter-spacing: var(--tracking-display);
		text-wrap: balance;
	}

	.accent {
		color: var(--color-accent);
	}

	p {
		color: var(--color-text-muted);
		text-wrap: pretty;
	}

	footer {
		margin-top: var(--space-18);
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
