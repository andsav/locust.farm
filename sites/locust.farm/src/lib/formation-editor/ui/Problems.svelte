<!-- The problems found, each with "Show me" and the technical details. Shown in the side panel. -->
<script lang="ts">
	import Icon from './Icon.svelte';
	import type { Place, Problem } from './problems.ts';

	let {
		problems,
		onshow
	}: {
		problems: Problem[];
		onshow: (place: Place) => void;
	} = $props();

	const errors = $derived(problems.filter((problem) => problem.fromLocust).length);
</script>

<div class="problems">
	{#if problems.length === 0}
		<p class="ok"><Icon name="check-circle" size={18} /> No problems found.</p>
	{:else}
		{#if errors > 0}
			<p class="faint">
				You can still copy the prompt. Your agent will save an unfinished draft and won't publish
				it.
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
								<summary>Details</summary>
								<code>{problem.technical}</code>
							</details>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.problems {
		display: grid;
		gap: 1rem;
	}

	.ok {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		color: var(--color-text-muted);
	}

	.ok :global(.icon) {
		color: var(--color-accent);
	}

	.faint {
		color: var(--color-text-subtle);
	}

	ul {
		display: grid;
		gap: 0.5rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		display: grid;
		gap: 0.5rem;
		padding: 0.75rem;
		border: 1px solid color-mix(in srgb, var(--color-accent) 50%, var(--color-border));
	}

	li.blocks {
		border-color: var(--color-accent);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: center;
	}

	summary {
		color: var(--color-text-subtle);
		cursor: pointer;
	}

	code {
		display: block;
		margin-top: 0.25rem;
		color: var(--color-text-subtle);
		font: var(--text-code);
		overflow-wrap: anywhere;
	}
</style>
