<script lang="ts">
	const links = [
		{ label: 'docs', href: '#' },
		{ label: 'protocol', href: '#' },
		{ label: 'github', href: 'https://github.com/andsav/locust.farm' }
	];
</script>

<header>
	<span class="brand">locust.farm</span>
	<nav aria-label="Primary">
		{#each links as { label, href } (label)}
			<a {href}>{label}</a>
		{/each}
	</nav>
</header>

<style>
	header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: var(--space-24) var(--gutter-inline);
		color: var(--color-text-subtle);
		font: var(--text-label);
		letter-spacing: var(--tracking-label);
		text-transform: uppercase;
	}

	.brand {
		color: var(--color-text);
	}

	nav {
		display: flex;
		gap: var(--space-24);
	}
</style>
