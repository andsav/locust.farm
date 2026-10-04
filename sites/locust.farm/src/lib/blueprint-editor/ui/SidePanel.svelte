<!--
	The settings panel that slides in over the right of the map. Adapted from
	Polaris's settings panel (crates/polaris/frontend/src/lib/components/blueprint/InspectorPanel.svelte
	at dreamcolor10 01d8aa3c4): a header with the accent disc and title, one
	scrolling body, and a footer band with the destructive action leading.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		open,
		number,
		title,
		onclose,
		children,
		footer
	}: {
		open: boolean;
		number: number;
		title: string;
		onclose: () => void;
		children: Snippet;
		footer?: Snippet;
	} = $props();

	const titleId = $props.id();
</script>

<section class="panel" class:open aria-labelledby={titleId} inert={!open}>
	<header>
		<span class="disc" aria-hidden="true">{number}</span>
		<h2 id={titleId}>{title}</h2>
		<button type="button" class="close" onclick={onclose}>Close</button>
	</header>
	<div class="content">
		{@render children()}
	</div>
	{#if footer}
		<footer>{@render footer()}</footer>
	{/if}
</section>

<style>
	.panel {
		position: absolute;
		top: 0;
		right: 0;
		bottom: 0;
		z-index: 20;
		display: flex;
		flex-direction: column;
		width: min(24rem, 100%);
		border-left: 1px solid var(--color-border);
		background: var(--color-bg);
		box-shadow: -18px 0 40px -24px rgb(0 0 0 / 0.9);
		transform: translateX(105%);
		transition: transform var(--duration-slow) cubic-bezier(0.16, 1, 0.3, 1);
	}

	.panel.open {
		transform: none;
	}

	header {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		min-height: 4rem;
		padding: 1rem 1rem 1rem 1.25rem;
		border-bottom: 1px solid var(--color-border);
	}

	.disc {
		display: grid;
		flex: none;
		place-items: center;
		width: 2.5rem;
		height: 2.5rem;
		border-radius: 50%;
		background: color-mix(in srgb, var(--color-accent) 18%, var(--color-surface));
		color: var(--color-accent);
		font: 500 0.875rem / 1 var(--font-mono);
	}

	h2 {
		flex: 1;
		min-width: 0;
		margin: 0;
		overflow: hidden;
		font: 500 0.9375rem / 1.3 var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.close {
		min-height: 2.25rem;
		padding: 0 0.75rem;
		border: var(--border-hairline);
		background: transparent;
		color: var(--color-text-muted);
		font: var(--text-ui);
		cursor: pointer;
	}

	.content {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: 1rem 1.25rem 1.5rem;
	}

	footer {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		min-height: 3.25rem;
		padding: 0.5rem 1.25rem;
		border-top: 1px solid var(--color-border);
	}
</style>
