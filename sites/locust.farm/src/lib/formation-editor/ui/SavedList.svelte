<!-- The formations saved in this browser. Shown in the side panel. -->
<script lang="ts">
	import type { SavedSummary } from '../storage/saved.ts';
	import Icon from './Icon.svelte';
	import { tip } from './tooltip.ts';

	let {
		saved,
		current,
		onopen,
		ondelete
	}: {
		saved: SavedSummary[];
		/** The record being edited. */
		current: string;
		onopen: (id: string) => void;
		ondelete: (id: string) => void;
	} = $props();
</script>

<div class="saved">
	{#if saved.length === 0}
		<p class="faint">Nothing saved yet.</p>
	{:else}
		<ul>
			{#each saved as item (item.id)}
				<li class:current={item.id === current}>
					<button type="button" class="name" onclick={() => onopen(item.id)}>
						<span>{item.name || 'Untitled formation'}</span>
						<span class="when">
							{item.origin === 'link'
								? 'From a link · '
								: item.origin === 'import'
									? 'Opened · '
									: ''}
							{new Date(item.saved).toLocaleString()}
						</span>
					</button>
					<button
						type="button"
						class="icon-button"
						aria-label={`Delete "${item.name || 'Untitled formation'}"`}
						use:tip={'Delete'}
						onclick={() => ondelete(item.id)}
					>
						<Icon name="trash" size={16} />
					</button>
				</li>
			{/each}
		</ul>
	{/if}
	<p class="faint">
		Browsers can clear this, and Safari does after seven days without a visit. Download or copy a
		link to keep a copy.
	</p>
</div>

<style>
	.saved {
		display: grid;
		gap: 1rem;
	}

	ul {
		display: grid;
		margin: 0;
		padding: 0;
		list-style: none;
		border-top: var(--border-hairline);
	}

	li {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		border-bottom: var(--border-hairline);
	}

	.name {
		display: grid;
		gap: 0.125rem;
		justify-items: start;
		min-height: 3.25rem;
		padding: 0.5rem 0.25rem;
		border: 0;
		background: transparent;
		text-align: left;
	}

	.name span:first-child {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	li.current .name span:first-child {
		color: var(--color-accent);
	}

	.when,
	.faint {
		color: var(--color-text-faint);
		font: var(--text-label);
	}

	li .icon-button {
		border-color: transparent;
		background: transparent;
	}
</style>
