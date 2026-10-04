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
<a class="skip-link button" href="#article">Skip to article</a>
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
		<p class="status eyebrow">{data.manifest.label} · {data.article.status}</p>
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
		gap: 3rem;
		max-width: 76rem;
		margin: auto;
		padding: 2.5rem var(--gutter-inline) 5rem;
	}
	aside nav,
	.toc {
		display: grid;
		gap: 0.5rem;
		color: var(--color-text-muted);
		font: var(--text-ui);
	}
	aside a:hover,
	.toc a:hover {
		color: var(--color-text);
	}
	aside h2 {
		margin: 1.5rem 0 0.125rem;
		color: var(--color-text-subtle);
		font: var(--text-ui-small);
		font-weight: 500;
	}
	aside [aria-current='page'] {
		color: var(--color-text);
		font-weight: 500;
	}
	main {
		min-width: 0;
	}
	main > nav:first-child {
		color: var(--color-text-subtle);
		font: var(--text-ui-small);
	}
	.status {
		margin: 1.5rem 0 0.75rem;
		color: var(--color-accent);
	}
	.identity {
		color: var(--color-text-subtle);
		font: var(--text-ui-small);
		overflow-wrap: anywhere;
	}
	.toc {
		margin: 1.5rem 0;
		padding: 1.5rem 0;
		border-block: var(--border-hairline);
	}
	.toc strong {
		color: var(--color-text);
		font: var(--text-ui-heading);
	}
	footer {
		margin-top: 3rem;
		padding-top: 1.5rem;
		border-top: var(--border-hairline);
		color: var(--color-text-muted);
		font: var(--text-ui);
	}
	.adjacent {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		gap: 1rem;
		margin-top: 1.5rem;
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
