<!-- What the agent should do, and the one copy button. -->
<script lang="ts">
	import { COPY_MESSAGES, copyText } from '../../onboarding/clipboard.ts';
	import type { BuiltPrompt, Intent } from '../prompt/prompt.ts';
	import Icon from './Icon.svelte';
	import type { IconName } from './icons.ts';
	import { tip } from './tooltip.ts';

	let {
		prompt,
		intent = $bindable(),
		hasErrors,
		blocked,
		onfail
	}: {
		prompt: BuiltPrompt | null;
		intent: Intent;
		hasErrors: boolean;
		/** Why copying waits, or null. */
		blocked: string | null;
		/** Copying failed: show the prompt so it can be copied by hand. */
		onfail: () => void;
	} = $props();

	let message = $state('');
	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	const choices: { value: Intent; icon: IconName; text: string; tip: () => string }[] = [
		{
			value: 'add',
			icon: 'plus',
			text: 'Add',
			tip: () =>
				hasErrors
					? 'Your agent saves it as an unfinished private draft and does not publish it.'
					: 'Your agent adds it to your Locust as a private draft and asks you before publishing.'
		},
		{
			value: 'check',
			icon: 'magnifying-glass',
			text: 'Check',
			tip: () => 'Your agent checks it with Locust and explains it. Nothing is saved.'
		}
	];

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
	<div class="intent" role="radiogroup" aria-label="What should your agent do?">
		{#each choices as choice (choice.value)}
			<button
				type="button"
				role="radio"
				aria-checked={intent === choice.value}
				use:tip={{ label: choice.text, description: choice.tip() }}
				onclick={() => (intent = choice.value)}
			>
				<Icon name={choice.icon} size={14} />
				{choice.text}
			</button>
		{/each}
	</div>
	<button
		type="button"
		class="primary"
		onclick={copy}
		disabled={!prompt || blocked !== null}
		use:tip={{
			label: 'Copy prompt',
			description: blocked ?? 'Paste it into your coding agent. It needs Locust on this computer.'
		}}
	>
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

	.intent {
		display: flex;
		border: 1px solid var(--color-border);
	}

	.intent button {
		display: flex;
		gap: 0.375rem;
		align-items: center;
		min-height: 2.25rem;
		padding: 0 0.625rem;
		border: 0;
		background: transparent;
		color: var(--color-text-subtle);
		font: var(--text-ui);
		cursor: pointer;
	}

	.intent button[aria-checked='true'] {
		background: var(--color-surface);
		color: var(--color-text);
	}

	.intent button:focus-visible {
		outline: 1px solid var(--color-accent);
		outline-offset: -2px;
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
