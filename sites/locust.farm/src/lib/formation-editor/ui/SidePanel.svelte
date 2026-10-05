<!--
	The settings panel slides in over the right of the map: a header with the
	accent disc and title, one
	scrolling body, and a footer band with the destructive action leading.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from './Icon.svelte';
	import type { IconName } from './icons.ts';
	import { tip } from './tooltip.ts';

	let {
		open,
		mark,
		title,
		onclose,
		children,
		footer
	}: {
		open: boolean;
		/** The stage's number, or an icon for the other panels. */
		mark: number | IconName;
		title: string;
		onclose: () => void;
		children: Snippet;
		footer?: Snippet;
	} = $props();

	const titleId = $props.id();
</script>

<section class="panel" class:open aria-labelledby={titleId} inert={!open}>
	<header>
		<span class="disc" aria-hidden="true">
			{#if typeof mark === 'number'}{mark}{:else}<Icon name={mark} size={18} />{/if}
		</span>
		<h2 id={titleId}>{title}</h2>
		<button
			type="button"
			class="close"
			aria-label="Close"
			use:tip={'Close (Esc)'}
			onclick={onclose}
		>
			<Icon name="x" size={18} />
		</button>
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
		width: min(25rem, 100%);
		border-left: 1px solid var(--color-border);
		background: var(--color-panel);
		box-shadow: -18px 0 40px -24px rgb(0 0 0 / 0.7);
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
		font: 600 0.875rem / 1 var(--font-sans);
	}

	h2 {
		flex: 1;
		min-width: 0;
		margin: 0;
		overflow: hidden;
		font: 600 1rem / 1.3 var(--font-sans);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.close {
		display: grid;
		place-items: center;
		width: 2.25rem;
		min-height: 2.25rem;
		padding: 0;
		border-color: transparent;
		background: transparent;
		color: var(--color-text-muted);
	}

	.close:hover {
		background: var(--color-surface-hover);
		color: var(--color-text);
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
