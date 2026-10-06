<!-- The formation in words: the page's plain summary, then Locust's own lines. Shown in the side panel. -->
<script lang="ts">
	import type { Explanation } from '../contract/types.ts';
	import type { SummaryLine } from '../model/words.ts';

	let {
		lines,
		explanation,
		hasSteps
	}: {
		lines: SummaryLine[];
		explanation: Explanation | null;
		hasSteps: boolean;
	} = $props();
</script>

<div class="summary">
	<ul class="lines">
		{#each lines as line, index (index)}
			<li>{line.text}</li>
		{/each}
		{#if hasSteps}
			<li>
				The host's Locust adds each step's task and sends it out. Locust never starts an agent.
			</li>
		{/if}
	</ul>

	<section class="locust" aria-labelledby="locust-title">
		<h3 id="locust-title">What Locust will say</h3>
		{#if explanation}
			<ul>
				{#each explanation.summary as line, index (index)}
					<li>{line}</li>
				{/each}
			</ul>
		{:else}
			<p class="faint">Locust won't explain it until the problems are fixed.</p>
		{/if}
	</section>
</div>

<style>
	.summary {
		display: grid;
		gap: 1.5rem;
	}

	.lines {
		display: grid;
		gap: 0.625rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	h3 {
		color: var(--color-text-subtle);
		font: var(--text-ui-small);
		font-weight: 500;
	}

	.locust ul {
		display: grid;
		gap: 0.375rem;
		margin: 0;
		padding-left: 1rem;
		color: var(--color-text-muted);
		font: var(--text-ui-small);
	}

	.faint {
		color: var(--color-text-faint);
	}
</style>
