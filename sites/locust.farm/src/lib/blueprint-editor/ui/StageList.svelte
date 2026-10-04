<!-- Every stage and arrow as text, with the same actions as the map. On phones it is the main view. -->
<script lang="ts">
	import type { EditorDocument } from '../model/document.ts';
	import { stageOrder } from '../model/edit.ts';
	import { EVIDENCE_WORDS, who } from '../model/words.ts';

	let {
		document,
		problems,
		onopen
	}: {
		document: EditorDocument;
		problems: Record<string, number>;
		onopen: (name: string) => void;
	} = $props();

	const order = $derived(stageOrder(document));
</script>

{#if order.length > 0}
	<section class="list" aria-labelledby="stage-list-title">
		<h3 id="stage-list-title">Stages in order</h3>
		<ol>
			{#each order as name (name)}
				{@const stage = document.blueprint.flow[name]}
				<li>
					<div class="line">
						<span class="name">{name}</span>
						{#if problems[name]}
							<span class="count"
								>{problems[name] === 1 ? '1 problem' : `${problems[name]} problems`}</span
							>
						{/if}
						<button type="button" onclick={() => onopen(name)}>Edit</button>
					</div>
					<p class="help">
						{stage.requires.length === 0
							? 'Can start any time.'
							: `Starts when ${stage.requires.map((item) => EVIDENCE_WORDS[item.evidence].when(item.stage)).join(' and ')}.`}
						Work goes to {who(stage.recipients)}.
					</p>
				</li>
			{/each}
		</ol>
	</section>
{/if}

<style>
	.list {
		display: grid;
		gap: 0.5rem;
	}

	h3 {
		margin: 0;
		font: 500 0.8125rem / 1.5 var(--font-mono);
	}

	ol {
		display: grid;
		gap: 0.5rem;
		margin: 0;
		padding-left: 1.25rem;
	}

	.line {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.name {
		font-weight: 500;
	}

	.count {
		padding: 0 0.375rem;
		border: 1px solid var(--color-accent);
		font: var(--text-label);
	}

	button {
		min-height: 2rem;
		margin-left: auto;
		padding: 0 0.625rem;
		border: var(--border-hairline);
		background: var(--color-surface);
		color: var(--color-text);
		font: var(--text-ui);
		cursor: pointer;
	}

	.help {
		color: var(--color-text-subtle);
	}
</style>
