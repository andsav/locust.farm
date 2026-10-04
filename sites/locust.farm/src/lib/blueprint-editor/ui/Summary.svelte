<!-- "What this means": the page's plain summary, the problems, and Locust's own words. -->
<script lang="ts">
	import type { Explanation } from '../contract/types.ts';
	import type { SummaryLine } from '../model/words.ts';
	import type { Place, Problem } from './problems.ts';

	let {
		lines,
		problems,
		explanation,
		hasStages,
		onshow
	}: {
		lines: SummaryLine[];
		problems: Problem[];
		explanation: Explanation | null;
		hasStages: boolean;
		onshow: (place: Place) => void;
	} = $props();

	const errors = $derived(problems.filter((problem) => problem.fromLocust).length);
</script>

<section class="summary" aria-labelledby="summary-title">
	<h2 id="summary-title">What this means</h2>
	<ul class="lines">
		{#each lines as line, index (index)}
			<li>{line.text}</li>
		{/each}
	</ul>
	{#if hasStages}
		<p class="fixed">
			When a stage is ready, the Locust of the person who runs it creates the task and sends it out.
			Locust never starts an agent.
		</p>
	{/if}

	<div class="problems" aria-live="polite">
		{#if problems.length === 0}
			<p class="ok">No problems found.</p>
		{:else}
			<h3>
				{problems.length === 1 ? '1 thing to check' : `${problems.length} things to check`}
			</h3>
			{#if errors > 0}
				<p class="help">
					You can still copy the prompt. Your agent will save it as an unfinished draft and won't
					publish it.
				</p>
			{/if}
			<ul>
				{#each problems as problem, index (index)}
					<li class:blocks={problem.blocksCopy}>
						<p>{problem.text}</p>
						<div class="actions">
							{#if problem.place}
								<button type="button" onclick={() => problem.place && onshow(problem.place)}>
									Show me
								</button>
							{/if}
							{#if problem.technical}
								<details>
									<summary>Technical details</summary>
									<code>{problem.technical}</code>
								</details>
							{:else}
								<span class="help">Checked by this page, not by Locust.</span>
							{/if}
						</div>
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<details class="locust">
		<summary>What Locust will say</summary>
		{#if explanation}
			<p class="help">Your agent shows these lines from Locust before it saves anything.</p>
			<ul>
				{#each explanation.summary as line, index (index)}
					<li>{line}</li>
				{/each}
			</ul>
		{:else}
			<p class="help">Locust won't explain the blueprint until the problems above are fixed.</p>
		{/if}
	</details>
</section>

<style>
	.summary {
		display: grid;
		gap: 0.75rem;
	}

	h2 {
		margin: 0;
		font: 500 0.8125rem / 1.5 var(--font-mono);
	}

	h3 {
		margin: 0;
		font: 500 0.8125rem / 1.5 var(--font-mono);
	}

	.lines {
		display: grid;
		gap: 0.5rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.fixed {
		padding-top: 0.5rem;
		border-top: var(--border-hairline);
		color: var(--color-text-subtle);
	}

	.problems ul {
		display: grid;
		gap: 0.5rem;
		margin: 0.5rem 0 0;
		padding: 0;
		list-style: none;
	}

	.problems li {
		padding: 0.625rem 0.75rem;
		border: 1px solid color-mix(in srgb, var(--color-accent) 50%, var(--color-border));
	}

	.problems li.blocks {
		border-color: var(--color-accent);
	}

	.ok {
		color: var(--color-text-subtle);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
		margin-top: 0.375rem;
	}

	.actions button {
		min-height: 2rem;
		padding: 0 0.625rem;
		border: var(--border-hairline);
		background: var(--color-surface);
		color: var(--color-text);
		font: var(--text-ui);
		cursor: pointer;
	}

	code {
		display: block;
		margin-top: 0.25rem;
		color: var(--color-text-subtle);
		font: var(--text-code);
		overflow-wrap: anywhere;
	}

	.locust ul {
		display: grid;
		gap: 0.375rem;
		margin: 0.5rem 0 0;
		padding-left: 1rem;
		color: var(--color-text-muted);
		font: var(--text-code);
	}

	.help {
		color: var(--color-text-subtle);
	}

	summary {
		cursor: pointer;
	}
</style>
