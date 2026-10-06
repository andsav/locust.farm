<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { copyText, COPY_MESSAGES } from '#lib/onboarding/clipboard.ts';

	interface Props {
		/** The prompt to display and copy. */
		text: string;
		/** Id of the prompt text element. */
		id: string;
	}

	let { text, id }: Props = $props();

	const FEEDBACK_MS = 1500;

	let hydrated = $state(false);
	let message = $state('');
	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let textEl: HTMLParagraphElement | undefined = $state();

	onMount(() => {
		hydrated = true;
	});

	async function copy() {
		// Clear first so a repeated identical message is re-announced.
		message = '';
		await tick();

		const result = await copyText(text, navigator.clipboard);

		if (result === 'copied') {
			message = COPY_MESSAGES.copied;
			copied = true;
			clearTimeout(timer);
			timer = setTimeout(() => (copied = false), FEEDBACK_MS);
		} else {
			message = COPY_MESSAGES.failed;
			copied = false;
			selectPrompt();
		}
	}

	function selectPrompt() {
		if (!textEl) return;
		const range = document.createRange();
		range.selectNodeContents(textEl);
		const sel = window.getSelection();
		sel?.removeAllRanges();
		sel?.addRange(range);
	}

	$effect(() => () => clearTimeout(timer));
</script>

<div class="prompt">
	<div class="controls">
		{#if hydrated}
			<button type="button" class="button primary" onclick={copy} aria-describedby={id}>
				{copied ? 'Copied' : 'Copy prompt'}
			</button>
		{/if}
		<p class="status" role="status" aria-live="polite">{message}</p>
	</div>
	<p class="text well" {id} bind:this={textEl}>{text}</p>
	<p class="hint">You can also select the text and copy it yourself.</p>
</div>

<style>
	.prompt {
		display: grid;
		gap: var(--space-12);
		max-width: 100%;
		min-width: 0;
	}

	.text {
		user-select: text;
		overflow-wrap: anywhere;
		white-space: normal;
	}

	.controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-16);
		min-height: 2.75rem;
	}

	.status {
		color: var(--color-text-muted);
		font: var(--text-ui-small);
	}

	.hint {
		color: var(--color-text-subtle);
		font: var(--text-ui-small);
	}
</style>
