<!-- "How does your team work?": six ways of working, each with its picture. -->
<script lang="ts">
	import { WAYS_OF_WORKING } from '../model/presets.ts';
	import { drawDiagram } from './diagrams.ts';

	let {
		current,
		onpick
	}: {
		current: string | null;
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
		<label class="way" class:current={current === way.id}>
			<input
				type="radio"
				{name}
				value={way.id}
				checked={current === way.id}
				onchange={() => onpick(way.id)}
			/>
			<svg viewBox="0 0 320 180" aria-hidden="true" use:draw={way.id}></svg>
			<span class="title">{way.title}</span>
			<span class="sentence">{way.sentence}</span>
		</label>
	{/each}
</fieldset>

<style>
	.ways {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 1px;
		margin: 0;
		padding: 0;
		border: 1px solid var(--color-border);
		background: var(--color-border);
	}

	@media (max-width: 64rem) {
		.ways {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 36rem) {
		.ways {
			grid-template-columns: 1fr;
		}
	}

	.way {
		position: relative;
		display: grid;
		align-content: start;
		gap: 0.25rem;
		padding: 0.875rem 1rem 1.125rem;
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
		margin-bottom: 0.5rem;
	}

	.title {
		font: 500 0.8125rem / 1.5 var(--font-mono);
	}

	.sentence {
		color: var(--color-text-muted);
	}

	svg :global(.d-edge) {
		fill: none;
		stroke: var(--color-border);
		stroke-width: 1;
	}

	svg :global(.d-node),
	svg :global(.d-stage) {
		fill: var(--color-surface);
		stroke: var(--color-text-faint);
		stroke-width: 1;
	}

	svg :global(.d-lead) {
		stroke: var(--color-accent);
	}

	svg :global(.d-label) {
		fill: var(--color-text-faint);
		font: 400 8.5px var(--font-mono);
		letter-spacing: 0.04em;
	}

	svg :global(.d-note) {
		fill: var(--color-accent);
	}

	svg :global(.d-stage-label) {
		fill: var(--color-text-muted);
		font: 400 9px var(--font-mono);
	}

	svg :global(.d-dot.d-work) {
		fill: var(--color-text-subtle);
	}

	svg :global(.d-dot.d-result) {
		fill: var(--color-text);
	}

	svg :global(.d-dot.d-decision) {
		fill: var(--color-accent);
	}

	svg :global(.d-ring) {
		fill: none;
		stroke-width: 1.2;
	}

	svg :global(.d-ring.d-decision) {
		stroke: var(--color-accent);
	}

	svg :global(.d-ring.d-busy) {
		stroke: var(--color-text-faint);
	}

	svg :global(.d-chosen) {
		fill: none;
		stroke: var(--color-accent);
		stroke-width: 1.4;
	}

	svg :global(.d-stage-active) {
		fill: none;
		stroke: var(--color-accent);
		stroke-width: 1.2;
	}

	svg :global(.d-arrow) {
		fill: none;
		stroke: var(--color-text-faint);
		stroke-width: 1;
	}
</style>
