<script lang="ts">
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let query = $state('');
	let searchPages = $state<{ title: string; url: string; status: string; text: string }[]>([]);
	let searchReady = $state(false);
	let searchError = $state(false);
	const matches = $derived(
		searchPages.filter((page) =>
			`${page.title} ${page.text}`.toLowerCase().includes(query.trim().toLowerCase())
		)
	);
	async function search() {
		if (searchReady) return;
		try {
			const response = await fetch('/docs/next/index.json');
			if (!response.ok) throw new Error('Search unavailable');
			searchPages = (await response.json()).pages;
			searchReady = true;
		} catch {
			searchError = true;
		}
	}
</script>

<svelte:head>
	<title>Docs — locust.farm</title>
	<meta
		name="description"
		content="Locust development documentation, concepts and offline blueprint authoring."
	/>
	<link rel="canonical" href="https://locust.farm/docs" />
</svelte:head>
<a class="skip" href="#main">Skip to documentation</a>
<SiteHeader />
<main id="main">
	<p class="label">{data.manifest.label}</p>
	<h1>Documentation.</h1>
	<p class="intro">
		Read the current development direction and authoring contract. Public installation is
		unavailable; this manual describes source capabilities and proposals.
	</p>
	<p>
		Source <a href={`${data.manifest.repository}/tree/${data.sourceCommit}`}
			>{data.sourceCommit.slice(0, 12)}</a
		>{data.sourceDirty ? ' · working tree includes uncommitted changes' : ''}.
		<a href="/start">Check your harness and installation status</a>.
	</p>
	<label class="search"
		>Search development documentation
		<input
			type="search"
			bind:value={query}
			onfocus={search}
			oninput={search}
			placeholder="Find a concept or command"
		/>
	</label>
	{#if query.trim()}
		<div aria-live="polite">
			{#if searchError}<p>Search is unavailable. Browse the articles below.</p>
			{:else if searchReady}
				<p>{matches.length} result{matches.length === 1 ? '' : 's'} · development</p>
				{#each matches as page (page.url)}<p>
						<a href={page.url}>{page.title}</a> <small>{page.status}</small>
					</p>{/each}
				{#if !matches.length}<p>No matching articles. Try a shorter term or browse below.</p>{/if}
			{/if}
		</div>
	{/if}
	<nav aria-label="Documentation articles">
		{#each data.manifest.pages as page (page.slug)}
			<a class="article" href={`/docs/next/${page.slug}`}
				><span class="label">{page.group} · {page.status}</span>
				<h2>{page.title}</h2>
				<p>{page.description}</p></a
			>
		{/each}
	</nav>
	<p>
		<a href="/docs/next/index.json">Machine-readable inventory</a> includes raw Markdown URLs, hashes
		and contract versions. No released documentation track is available.
	</p>
</main>

<style>
	main {
		max-width: 64rem;
		padding: 3rem var(--gutter-inline) 5rem;
		margin: auto;
		font:
			1rem/1.8 system-ui,
			sans-serif;
	}
	h1 {
		font-size: clamp(2rem, 5vw, 3.5rem);
		line-height: 1.2;
		margin: 1rem 0 2rem;
	}
	p {
		margin: 1rem 0;
	}
	.intro {
		font-size: 1.2rem;
		max-width: 48rem;
	}
	.label {
		color: var(--color-text-muted);
		font: var(--text-label);
	}
	main a {
		text-decoration: underline;
		text-underline-offset: 0.2em;
	}
	.search {
		display: grid;
		gap: 0.5rem;
		margin: 2rem 0;
	}
	input {
		padding: 0.8rem;
		font: inherit;
		color: inherit;
		background: var(--color-bg);
		border: var(--border-hairline);
		border-radius: 0.3rem;
	}
	.article {
		display: block;
		padding: 1.5rem 0;
		border-top: var(--border-hairline);
		text-decoration: none;
	}
	h2 {
		margin: 0.4rem 0;
		font-size: 1.4rem;
	}
	.article p {
		margin: 0;
		color: var(--color-text-muted);
	}
	.skip {
		position: absolute;
		left: 1rem;
		top: -5rem;
		z-index: 20;
		background: var(--color-bg);
		padding: 1rem;
	}
	.skip:focus {
		top: 1rem;
	}
</style>
