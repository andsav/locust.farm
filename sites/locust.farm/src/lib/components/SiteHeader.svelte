<script lang="ts">
	import { page } from '$app/state';
	import { HOME_PATH, NAV_LINKS } from '#lib/site.ts';

	const current = (href: string) => (page.url.pathname === href ? 'page' : undefined);
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
		row-gap: var(--space-12);
		padding: var(--space-24) var(--gutter-inline);
		color: var(--color-text-subtle);
		font: var(--text-label);
		letter-spacing: var(--tracking-label);
		text-shadow: var(--text-halo);
		text-transform: uppercase;
	}

	.brand {
		color: var(--color-text);
	}

	nav {
		display: flex;
		gap: var(--space-24);
	}

	/* The labels are small, so each link's hit area extends past its text without moving it. */
	a {
		position: relative;
	}

	a::after {
		content: '';
		position: absolute;
		inset: calc(-1 * var(--space-14)) calc(-1 * var(--space-12) / 2);
	}

	nav [aria-current='page'] {
		color: var(--color-text);
	}
</style>
