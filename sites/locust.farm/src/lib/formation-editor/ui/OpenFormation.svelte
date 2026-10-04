<!-- Open a formation from pasted text or a file. Shown in the side panel. -->
<script lang="ts">
	import Icon from './Icon.svelte';

	let {
		message,
		onopen
	}: {
		/** Why the last attempt failed, or an empty string. */
		message: string;
		onopen: (text: string) => void;
	} = $props();

	let pasted = $state('');

	async function openFile(event: Event) {
		const file = (event.target as HTMLInputElement).files?.[0];
		if (file) onopen(await file.text());
	}
</script>

<div class="open">
	<textarea
		rows="10"
		bind:value={pasted}
		aria-label="Pasted formation"
		placeholder="Paste formation JSON, a prompt from this page, or your agent's reply."></textarea>
	<div class="actions">
		<button type="button" onclick={() => onopen(pasted)} disabled={pasted.trim() === ''}>
			Open
		</button>
		<label class="file">
			<Icon name="folder-open" size={16} />
			<span>Choose a file</span>
			<input type="file" accept=".json,.txt,application/json,text/plain" onchange={openFile} />
		</label>
	</div>
	{#if message}
		<p class="message" role="alert">{message}</p>
	{/if}
</div>

<style>
	.open {
		display: grid;
		gap: 0.75rem;
	}

	.actions {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.file {
		position: relative;
		display: flex;
		gap: 0.5rem;
		align-items: center;
		min-height: 2.25rem;
		padding: 0 0.75rem;
		color: var(--color-text-muted);
		cursor: pointer;
	}

	.file:hover,
	.file:focus-within {
		color: var(--color-text);
	}

	.file:focus-within {
		outline: 1px solid var(--color-accent);
	}

	.file input {
		position: absolute;
		inset: 0;
		opacity: 0;
		cursor: pointer;
	}

	.message {
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--color-accent);
	}
</style>
