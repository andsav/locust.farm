<script lang="ts">
	import type { LayoutProps } from './$types';
	import '@fontsource-variable/martian-mono/wght.css';
	import '#lib/styles/tokens.css';
	import '#lib/styles/base.css';
	import martianMonoLatin from '@fontsource-variable/martian-mono/files/martian-mono-latin-wght-normal.woff2?url';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="preload" href={martianMonoLatin} as="font" type="font/woff2" crossorigin="anonymous" />
</svelte:head>

{@render children()}
