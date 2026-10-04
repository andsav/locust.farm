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
	<p class="text" {id} bind:this={textEl}>{text}</p>
	<div class="controls">
		{#if hydrated}
			<button type="button" onclick={copy} aria-describedby={id}>
				{copied ? 'copied' : 'copy prompt'}
			</button>
		{/if}
		<p class="status" role="status" aria-live="polite">{message}</p>
	</div>
	<p class="hint">You can also select the text and copy it yourself.</p>
</div>

<style>
	.prompt {
		display: grid;
		gap: var(--space-12);
		max-width: 100%;
	}

	.text {
		margin: 0;
		padding: var(--space-16);
		border: var(--border-hairline);
		background: var(--color-bg);
		color: var(--color-text);
		font: var(--text-code);
		line-height: 1.7;
		user-select: text;
		overflow-wrap: anywhere;
		white-space: normal;
	}

	.controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-16);
	}

	button {
		min-height: 40px;
		padding: var(--space-12) var(--space-16);
		border: 1px solid var(--color-accent);
		background: transparent;
		color: var(--color-text);
		font: var(--text-label);
		letter-spacing: var(--tracking-label);
		text-transform: uppercase;
		cursor: pointer;
		transition-property: scale, background-color, color;
		transition-duration: 150ms;
		transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
	}

	button:hover {
		background: var(--color-accent);
		color: var(--color-bg);
	}

	button:active {
		scale: 0.96;
	}

	@media (prefers-reduced-motion: reduce) {
		button {
			transition: none;
		}
		button:active {
			scale: none;
		}
	}

	.status {
		margin: 0;
		color: var(--color-text-muted);
		font: var(--text-label);
	}

	.hint {
		margin: 0;
		color: var(--color-text-faint);
		font: var(--text-label);
	}
</style>
