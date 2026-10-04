<!-- "How does your team work?": six ways of working, each with its picture. The sentence is the tooltip. -->
<script lang="ts">
	import { WAYS_OF_WORKING } from '../model/presets.ts';
	import { drawDiagram } from './diagrams.ts';
	import { tip } from './tooltip.ts';

	let {
		current,
		onpick
	}: {
		/** The way of working whose rules equal the current rules, if any. */
		current: string | null;
		onpick: (id: string) => void;
	} = $props();

	function draw(svg: SVGSVGElement, id: string) {
		drawDiagram(svg, id);
	}
</script>

<div class="ways" role="group" aria-label="Ways of working">
	{#each WAYS_OF_WORKING as way (way.id)}
		<button
			type="button"
			class="way"
			class:current={current === way.id}
			aria-pressed={current === way.id}
			use:tip={{ label: way.title, description: way.sentence }}
			onclick={() => onpick(way.id)}
		>
			<svg class="diagram" viewBox="0 0 320 180" aria-hidden="true" use:draw={way.id}></svg>
			<span class="title">{way.title}</span>
			<span class="visually-hidden">{way.sentence}</span>
		</button>
	{/each}
</div>

<style>
	.ways {
		display: grid;
		grid-template-columns: repeat(6, minmax(0, 1fr));
		gap: 0.5rem;
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
		min-height: 0;
		padding: 0.5rem 0.75rem 0.75rem;
		border: 1px solid transparent;
		background: var(--color-panel);
		color: var(--color-text);
		text-align: left;
	}

	.way:hover {
		background: var(--color-surface);
	}

	.way.current {
		border-color: var(--color-accent);
		background: var(--color-surface);
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
		font: var(--text-ui);
		font-weight: 500;
	}

	.way:not(.current) .title {
		color: var(--color-text-muted);
	}
</style>
