<script lang="ts">
	import { codeCopy } from '#lib/docs/code-copy.ts';
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	const index = $derived(data.manifest.pages.findIndex((page) => page.slug === data.article.slug));
	const previous = $derived(data.manifest.pages[index - 1]);
	const next = $derived(data.manifest.pages[index + 1]);
</script>

<svelte:head>
	<title>{data.article.title} — Locust docs</title>
	<meta name="description" content={data.article.description} />
	<link rel="canonical" href={`https://locust.farm${data.article.url}`} />
</svelte:head>
<a class="skip" href="#article">Skip to article</a>
<SiteHeader />
<div class="shell">
	<aside>
		<nav aria-label="Documentation navigation">
			<a href="/docs">All documentation</a>
			{#each [...new Set(data.manifest.pages.map((page) => page.group))] as group (group)}
				<h2>{group}</h2>
				{#each data.manifest.pages.filter((page) => page.group === group) as page (page.slug)}
					<a
						href={`/docs/next/${page.slug}`}
						aria-current={page.slug === data.article.slug ? 'page' : undefined}>{page.title}</a
					>
				{/each}
			{/each}
		</nav>
	</aside>
	<main id="article">
		<nav aria-label="Breadcrumb"><a href="/docs">Docs</a> / Development / {data.article.title}</nav>
		{#if data.article.section}<p>
				This subject is covered in <a href={`#${data.article.section}`}
					>{data.article.headings.find((heading) => heading.id === data.article.section)?.title}</a
				>.
			</p>{/if}
		<p class="status">{data.manifest.label} · {data.article.status}</p>
		<p class="identity">
			Source <a href={data.article.sourceUrl}>{data.sourceCommit.slice(0, 12)}</a>{data.sourceDirty
				? ' · uncommitted working tree'
				: ''}<br />
			Software: {data.manifest.versions.software} · API: {data.manifest.versions.api} · Protocol: {data
				.manifest.versions.protocol} · Formation schema: {data.manifest.versions.formationSchema}
		</p>
		<nav class="toc" aria-label="On this page">
			<strong>On this page</strong
			>{#each data.article.headings.filter((heading) => heading.level === 2) as heading (heading.id)}<a
					href={`#${heading.id}`}>{heading.title}</a
				>{/each}
		</nav>
		<!-- HTML comes only from reviewed canonical Markdown: raw HTML disabled and link targets validated. -->
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		<div class="prose" use:codeCopy={data.article.html}>{@html data.article.html}</div>
		<footer>
			<a href={data.article.rawUrl}>Raw Markdown</a> ·
			<a href="/docs/next/index.json">Version inventory</a>
			<div class="adjacent">
				{#if previous}<a href={`/docs/next/${previous.slug}`}>← {previous.title}</a
					>{/if}{#if next}<a href={`/docs/next/${next.slug}`}>{next.title} →</a>{/if}
			</div>
		</footer>
	</main>
</div>

<style>
	.shell {
		display: grid;
		grid-template-columns: 15rem minmax(0, 52rem);
		max-width: 76rem;
		margin: auto;
		gap: 3rem;
		padding: 2.5rem var(--gutter-inline) 5rem;
		font:
			1rem/1.8 system-ui,
			sans-serif;
	}
	aside nav,
	.toc {
		display: grid;
		gap: 0.65rem;
	}
	aside h2 {
		font-size: 0.85rem;
		color: var(--color-text-muted);
		margin: 1.8rem 0 0.2rem;
	}
	[aria-current='page'] {
		color: var(--color-accent);
	}
	main {
		min-width: 0;
	}
	main a {
		text-decoration: underline;
		text-underline-offset: 0.2em;
	}
	.status {
		margin: 1rem 0;
		color: var(--color-accent);
	}
	.identity {
		color: var(--color-text-muted);
		font-size: 0.85rem;
		overflow-wrap: anywhere;
	}
	.toc {
		padding: 1.5rem 0;
		margin: 1.5rem 0;
		border-block: var(--border-hairline);
	}
	.prose :global(h1) {
		font-size: clamp(2rem, 5vw, 3rem);
		line-height: 1.2;
		margin: 2rem 0;
	}
	.prose :global(h2) {
		font-size: 1.6rem;
		margin: 2.5rem 0 1rem;
		scroll-margin-top: 1rem;
	}
	.prose :global(p),
	.prose :global(ul) {
		margin: 1.2rem 0;
	}
	.prose :global(code) {
		font: 0.9em/1.7 monospace;
	}
	.prose :global(pre) {
		padding: 1rem;
		overflow-x: auto;
		border: var(--border-hairline);
	}
	.prose :global(table) {
		display: block;
		overflow-x: auto;
		border-collapse: collapse;
	}
	.prose :global(th),
	.prose :global(td) {
		padding: 0.75rem;
		text-align: left;
		border: var(--border-hairline);
		min-width: 10rem;
	}
	.prose :global(.code-controls) {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 1rem;
		margin: 0.5rem 0 1.5rem;
	}
	.prose :global(.code-controls button) {
		color: inherit;
		background: var(--color-bg);
		border: 1px solid var(--color-accent);
		padding: 0.6rem 1rem;
		font: inherit;
		cursor: pointer;
	}
	footer {
		border-top: var(--border-hairline);
		margin-top: 3rem;
		padding-top: 1.5rem;
	}
	.adjacent {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		gap: 1rem;
		margin-top: 1.5rem;
	}
	.skip {
		position: absolute;
		top: -5rem;
		left: 1rem;
		padding: 1rem;
		background: var(--color-bg);
		z-index: 20;
	}
	.skip:focus {
		top: 1rem;
	}
	@media (max-width: 800px) {
		.shell {
			grid-template-columns: minmax(0, 1fr);
			gap: 2rem;
		}
		aside nav {
			display: flex;
			flex-wrap: wrap;
			gap: 0.7rem 1.2rem;
		}
		aside h2 {
			width: 100%;
			margin: 0.7rem 0 0;
		}
	}
</style>
