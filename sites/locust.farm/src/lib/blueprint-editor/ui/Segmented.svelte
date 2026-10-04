<!-- A row of icon choices that behaves as one radio group. Each choice's sentence is in its tooltip. -->
<script lang="ts" module>
	import type { IconName } from './icons.ts';

	export interface Option<V extends string> {
		value: V;
		icon: IconName;
		text: string;
		/** One sentence, shown as the tooltip. */
		tip: string;
		disabled?: boolean;
	}
</script>

<script lang="ts" generics="T extends string">
	import Icon from './Icon.svelte';
	import { tip } from './tooltip.ts';

	let {
		label,
		options,
		value,
		onchange
	}: {
		label: string;
		options: Option<T>[];
		value: T;
		onchange: (value: T) => void;
	} = $props();

	let group: HTMLDivElement | undefined = $state();

	function onkeydown(event: KeyboardEvent) {
		const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
		if (!step) return;
		event.preventDefault();
		const enabled = options.filter((option) => !option.disabled);
		const at = enabled.findIndex((option) => option.value === value);
		const next = enabled[(at + step + enabled.length) % enabled.length];
		onchange(next.value);
		queueMicrotask(() =>
			group?.querySelector<HTMLElement>(`[data-value="${CSS.escape(next.value)}"]`)?.focus()
		);
	}
</script>

<div class="segmented" role="radiogroup" aria-label={label} bind:this={group}>
	{#each options as option (option.value)}
		<button
			type="button"
			role="radio"
			data-value={option.value}
			aria-checked={value === option.value}
			tabindex={value === option.value ? 0 : -1}
			disabled={option.disabled}
			use:tip={{ label: option.text, description: option.tip }}
			onclick={() => onchange(option.value)}
			{onkeydown}
		>
			<Icon name={option.icon} size={20} />
			<span>{option.text}</span>
		</button>
	{/each}
</div>

<style>
	.segmented {
		display: grid;
		grid-auto-columns: minmax(0, 1fr);
		grid-auto-flow: column;
		gap: 1px;
		border: 1px solid var(--color-border);
		background: var(--color-border);
	}

	button {
		display: grid;
		justify-items: center;
		gap: 0.375rem;
		min-height: 3.75rem;
		padding: 0.625rem 0.25rem 0.5rem;
		border: 0;
		background: var(--color-bg);
		color: var(--color-text-subtle);
		font: var(--text-label);
		cursor: pointer;
		transition:
			color var(--duration-fast),
			background var(--duration-fast);
	}

	button:hover:not(:disabled) {
		color: var(--color-text);
		background: color-mix(in srgb, var(--color-surface) 60%, var(--color-bg));
	}

	button[aria-checked='true'] {
		background: var(--color-surface);
		color: var(--color-text);
		box-shadow: inset 0 -2px 0 var(--color-accent);
	}

	button[aria-checked='true'] :global(.icon) {
		color: var(--color-accent);
	}

	button:disabled:not([aria-checked='true']) {
		opacity: 0.4;
		cursor: not-allowed;
	}

	button:focus-visible {
		position: relative;
		outline: 1px solid var(--color-accent);
		outline-offset: -3px;
	}

	span {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
