<script lang="ts">
	import { NAV_LINKS } from '#lib/site.ts';
	let { gallery = false }: { gallery?: boolean } = $props();
</script>

<header class="site-header">
	<a class="brand" href="/">locust.farm</a>
	<nav aria-label="Primary">
		{#each NAV_LINKS as link (link.href)}
			<a href={link.href} aria-current={gallery && link.href === '/farms' ? 'page' : undefined}
				>{link.label}</a
			>
		{/each}
	</nav>
</header>
