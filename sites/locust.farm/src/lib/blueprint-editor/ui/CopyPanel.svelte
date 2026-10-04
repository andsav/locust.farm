<!-- What the agent should do, and the one copy button. -->
<script lang="ts">
	import { tick } from 'svelte';
	import { COPY_MESSAGES, copyText } from '../../onboarding/clipboard.ts';
	import type { EditorDocument } from '../model/document.ts';
	import { buildPrompt, LARGE_PROMPT, type BuiltPrompt, type Intent } from '../prompt/prompt.ts';

	let {
		document,
		hasErrors,
		blocked
	}: {
		document: EditorDocument;
		hasErrors: boolean;
		/** Why copying waits, or null. */
		blocked: string | null;
	} = $props();

	let intent = $state<Intent>('add');
	let prompt = $state<BuiltPrompt | null>(null);
	let message = $state('');
	let copied = $state(false);
	let promptElement: HTMLPreElement | undefined = $state();
	const name = $props.id();

	$effect(() => {
		const current = { document, intent, hasErrors };
		let cancelled = false;
		buildPrompt(current.document, current.intent, current.hasErrors).then((built) => {
			if (!cancelled) prompt = built;
		});
		return () => {
			cancelled = true;
		};
	});

	const size = $derived(
		prompt ? `${(prompt.text.length / 1000).toFixed(1)} thousand characters` : ''
	);

	async function copy() {
		if (!prompt || blocked) return;
		message = '';
		await tick();
		const result = await copyText(prompt.text, navigator.clipboard);
		if (result === 'copied') {
			message = COPY_MESSAGES.copied;
			copied = true;
			setTimeout(() => (copied = false), 1500);
		} else {
			message = COPY_MESSAGES.failed;
			copied = false;
			const details = promptElement?.closest('details');
			if (details) details.open = true;
			if (promptElement) {
				const range = window.document.createRange();
				range.selectNodeContents(promptElement);
				const selection = window.getSelection();
				selection?.removeAllRanges();
				selection?.addRange(range);
			}
		}
	}
</script>

<section class="copy" aria-labelledby="copy-title">
	<h2 id="copy-title">Copy prompt</h2>
	<fieldset>
		<legend>What should your agent do?</legend>
		<label class="choice">
			<input type="radio" {name} value="check" bind:group={intent} />
			<span>
				<span class="choice-title">Check it</span>
				<span class="help">Your agent checks it with Locust and explains it. Nothing is saved.</span
				>
			</span>
		</label>
		<label class="choice">
			<input type="radio" {name} value="add" bind:group={intent} />
			<span>
				<span class="choice-title">Add it to my Locust</span>
				<span class="help">
					{hasErrors
						? 'Your agent saves it as an unfinished private draft and does not publish it.'
						: 'Your agent saves it as a private draft and asks you before publishing.'}
				</span>
			</span>
		</label>
	</fieldset>

	<button type="button" class="primary" onclick={copy} disabled={!prompt || blocked !== null}>
		{copied ? 'Copied' : 'Copy prompt'}
	</button>
	{#if blocked}
		<p class="help">{blocked}</p>
	{/if}
	<p class="status" role="status">{message}</p>
	<p class="help">
		Paste it into your coding agent. It needs Locust on this computer.
		<a href="/start">Not set up yet? Start here.</a>
	</p>
	{#if prompt && prompt.text.length > LARGE_PROMPT}
		<p class="help">
			This prompt is long. Some agents shorten very long pastes; if yours does, use Download and
			give your agent the file instead.
		</p>
	{/if}

	<details>
		<summary>See the prompt{size ? ` (${size})` : ''}</summary>
		<pre bind:this={promptElement}>{prompt?.text ?? ''}</pre>
	</details>
</section>

<style>
	.copy {
		display: grid;
		gap: 0.75rem;
	}

	h2 {
		margin: 0;
		font: 500 0.8125rem / 1.5 var(--font-mono);
	}

	fieldset {
		display: grid;
		gap: 0.5rem;
		margin: 0;
		padding: 0;
		border: 0;
	}

	legend {
		margin-bottom: 0.25rem;
		color: var(--color-text-muted);
	}

	.primary {
		min-height: 2.75rem;
		border: 1px solid var(--color-accent);
		background: var(--color-accent);
		color: var(--color-bg);
		font: 500 0.8125rem / 1 var(--font-mono);
		letter-spacing: var(--tracking-label);
		text-transform: uppercase;
		cursor: pointer;
	}

	.primary:disabled {
		border-color: var(--color-border);
		background: transparent;
		color: var(--color-text-faint);
		cursor: not-allowed;
	}

	.status:empty {
		display: none;
	}

	pre {
		max-height: 20rem;
		overflow: auto;
		margin: 0.5rem 0 0;
		padding: 0.75rem;
		border: var(--border-hairline);
		color: var(--color-text-muted);
		font: var(--text-code);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	a {
		text-decoration: underline;
	}

	summary {
		cursor: pointer;
	}
</style>
