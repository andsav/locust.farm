<script lang="ts">
	interface Props {
		/** The shell command to show and copy, without the prompt. */
		command: string;
	}

	let { command }: Props = $props();

	const FEEDBACK_MS = 1500;

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function copy() {
		try {
			await navigator.clipboard.writeText(command);
		} catch {
			// Clipboard access is unavailable or was denied, so there is nothing to confirm.
			return;
		}
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), FEEDBACK_MS);
	}

	$effect(() => () => clearTimeout(timer));
</script>

<button type="button" onclick={copy}>
	<span><span aria-hidden="true">$</span> <code>{command}</code></span>
	<span class="label">{copied ? 'copied' : 'copy'}</span>
</button>
<span class="visually-hidden" role="status">{copied ? 'Copied to clipboard' : ''}</span>

<style>
	button {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-14);
		align-items: center;
		max-width: 100%;
		padding: var(--space-12) var(--space-16);
		border: var(--border-hairline);
		background: transparent;
		color: var(--color-text);
		font: var(--text-code);
		text-shadow: var(--text-halo);
		cursor: pointer;
	}

	button:hover {
		border-color: var(--color-accent);
	}

	.label {
		color: var(--color-text-faint);
	}
</style>
