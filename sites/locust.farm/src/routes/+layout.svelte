<script lang="ts">
	import type { LayoutProps } from './$types';
	import '#lib/styles/fonts.css';
	import '#lib/styles/tokens.css';
	import '#lib/styles/base.css';
	import martianMonoLatin from '@fontsource-variable/martian-mono/files/martian-mono-latin-wght-normal.woff2?url';
	import geistLatin from '@fontsource-variable/geist/files/geist-latin-wght-normal.woff2?url';
	import majorMonoLatin from '@fontsource/major-mono-display/files/major-mono-display-latin-400-normal.woff2?url';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="preload" href={martianMonoLatin} as="font" type="font/woff2" crossorigin="anonymous" />
	<link rel="preload" href={geistLatin} as="font" type="font/woff2" crossorigin="anonymous" />
	<link rel="preload" href={majorMonoLatin} as="font" type="font/woff2" crossorigin="anonymous" />
</svelte:head>

{@render children()}
