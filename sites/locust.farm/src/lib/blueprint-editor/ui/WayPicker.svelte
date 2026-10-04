<!-- "How does your team work?": six ways of working, each with its picture. The sentence is the tooltip. -->
<script lang="ts">
	import { WAYS_OF_WORKING } from '../model/presets.ts';
	import { drawDiagram } from './diagrams.ts';
	import { tip } from './tooltip.ts';

	let {
		current,
		changed,
		onpick
	}: {
		current: string | null;
		/** How many settings differ from the current way of working. */
		changed: number;
		onpick: (id: string) => void;
	} = $props();

	const name = $props.id();

	function draw(svg: SVGSVGElement, id: string) {
		drawDiagram(svg, id);
	}
</script>

<fieldset class="ways">
	<legend class="visually-hidden">Ways of working</legend>
	{#each WAYS_OF_WORKING as way (way.id)}
		<label
			class="way"
			class:current={current === way.id}
			use:tip={{
				label: way.title,
				description:
					current === way.id && changed > 0
						? `${way.sentence} Click to go back to it; undo keeps your changes.`
						: way.sentence
			}}
		>
			<input
				type="radio"
				{name}
				value={way.id}
				checked={current === way.id}
				onchange={() => onpick(way.id)}
				onclick={() => {
					if (current === way.id && changed > 0) onpick(way.id);
				}}
			/>
			<svg class="diagram" viewBox="0 0 320 180" aria-hidden="true" use:draw={way.id}></svg>
			<span class="title">
				{way.title}
				{#if current === way.id && changed > 0}
					<span class="changed">changed</span>
				{/if}
			</span>
			<span class="visually-hidden">{way.sentence}</span>
		</label>
	{/each}
</fieldset>

<style>
	.ways {
		display: grid;
		grid-template-columns: repeat(6, minmax(0, 1fr));
		gap: 1px;
		margin: 0;
		padding: 0;
		border: 1px solid var(--color-border);
		background: var(--color-border);
	}

	@media (max-width: 72rem) {
		.ways {
			grid-template-columns: repeat(3, minmax(0, 1fr));
		}
	}

	@media (max-width: 36rem) {
		.ways {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	.way {
		position: relative;
		display: grid;
		align-content: start;
		gap: 0.25rem;
		padding: 0.5rem 0.75rem 0.75rem;
		background: var(--color-bg);
		cursor: pointer;
	}

	.way:hover {
		background: color-mix(in srgb, var(--color-surface) 50%, var(--color-bg));
	}

	.way.current {
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-accent);
	}

	.way:has(input:focus-visible) {
		outline: 1px solid var(--color-accent);
		outline-offset: -4px;
	}

	input {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}

	svg {
		display: block;
		width: 100%;
		height: auto;
	}

	.title {
		display: flex;
		gap: 0.5rem;
		align-items: baseline;
		font: 500 0.8125rem / 1.5 var(--font-mono);
	}

	.way:not(.current) .title {
		color: var(--color-text-muted);
	}

	.changed {
		color: var(--color-accent);
		font: var(--text-label);
	}
</style>
