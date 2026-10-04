<!-- The one copy button. -->
<script lang="ts">
	import { COPY_MESSAGES, copyText } from '../../onboarding/clipboard.ts';
	import type { BuiltPrompt } from '../prompt/prompt.ts';
	import Icon from './Icon.svelte';

	let {
		prompt,
		blocked,
		onfail
	}: {
		prompt: BuiltPrompt | null;
		/** Why copying waits, or null. */
		blocked: string | null;
		/** Copying failed: show the prompt so it can be copied by hand. */
		onfail: () => void;
	} = $props();

	let message = $state('');
	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function copy() {
		if (!prompt || blocked) return;
		clearTimeout(timer);
		const result = await copyText(prompt.text, navigator.clipboard);
		if (result === 'copied') {
			message = COPY_MESSAGES.copied;
			copied = true;
		} else {
			message = COPY_MESSAGES.failed;
			copied = false;
			onfail();
		}
		timer = setTimeout(() => {
			copied = false;
			message = '';
		}, 5000);
	}
</script>

<div class="copy">
	<button type="button" class="primary" onclick={copy} disabled={!prompt || blocked !== null}>
		<Icon name={copied ? 'check' : 'copy'} size={16} />
		{copied ? 'Copied' : 'Copy prompt'}
	</button>
	<p class="status" role="status">{message}</p>
</div>

<style>
	.copy {
		position: relative;
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.primary {
		display: flex;
		flex: 1 0 auto;
		justify-content: center;
		gap: 0.5rem;
		align-items: center;
		min-height: 2.5rem;
		padding: 0 1rem;
		border: 1px solid var(--color-accent);
		background: var(--color-accent);
		color: var(--color-bg);
		font: 500 0.8125rem / 1 var(--font-mono);
		letter-spacing: var(--tracking-label);
		text-transform: uppercase;
		white-space: nowrap;
		cursor: pointer;
	}

	.primary:hover:not(:disabled) {
		border-color: var(--color-text);
	}

	.primary:disabled {
		border-color: var(--color-border);
		background: transparent;
		color: var(--color-text-faint);
		cursor: not-allowed;
	}

	.status {
		position: absolute;
		top: calc(100% + 0.5rem);
		right: 0;
		z-index: 30;
		width: max-content;
		max-width: 22rem;
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--color-accent);
		background: var(--color-bg);
	}

	.status:empty {
		display: none;
	}
</style>
