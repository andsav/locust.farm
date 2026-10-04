<script lang="ts">
	import { page } from '$app/state';
	import { HOME_PATH, NAV_LINKS } from '#lib/site.ts';

	/** A section's link stays current on the pages beneath it. */
	const current = (href: string) =>
		page.url.pathname === href || (href !== HOME_PATH && page.url.pathname.startsWith(`${href}/`))
			? 'page'
			: undefined;
</script>

<header>
	<a class="brand" href={HOME_PATH} aria-current={current(HOME_PATH)}>locust.farm</a>
	<nav aria-label="Primary">
		{#each NAV_LINKS as { label, href } (label)}
			<a {href} aria-current={current(href)}>{label}</a>
		{/each}
	</nav>
</header>

<style>
	header {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-12);
		padding: var(--space-18) var(--gutter-inline);
		text-shadow: var(--text-halo);
	}

	/* The wordmark stays in mono, as the name is written everywhere else. */
	.brand {
		font: var(--text-code);
		font-weight: 500;
	}

	nav {
		display: flex;
		gap: 0.125rem;
	}

	nav a {
		padding: 0.375rem 0.625rem;
		border-radius: var(--radius-control);
		color: var(--color-text-subtle);
		font: var(--text-ui);
	}

	nav a:hover {
		color: var(--color-text);
	}

	nav [aria-current='page'] {
		background: var(--color-surface);
		color: var(--color-text);
		text-shadow: none;
	}
</style>
