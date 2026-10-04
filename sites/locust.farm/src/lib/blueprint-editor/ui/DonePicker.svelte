<!-- "When is a task done?" as plain choices, each with one line of explanation. -->
<script lang="ts">
	import type { CompletionRule, Selector } from '../contract/types.ts';
	import { doneAnswer, doneRule, type DoneAnswer } from '../model/answers.ts';
	import { doneClause } from '../model/words.ts';
	import WhoPicker from './WhoPicker.svelte';

	let {
		value,
		roles,
		legend,
		onchange
	}: {
		value: CompletionRule;
		roles: string[];
		legend: string;
		onchange: (value: CompletionRule) => void;
	} = $props();

	const name = $props.id();
	const answer = $derived(doneAnswer(value));

	function pick(kind: DoneAnswer['kind']) {
		const members: Selector = { kind: 'members' };
		if (kind === 'self') onchange(doneRule({ kind: 'self' }, value));
		if (kind === 'review') {
			onchange(doneRule({ kind: 'review', by: members, count: 1, excludeAuthor: true }, value));
		}
		if (kind === 'check') onchange(doneRule({ kind: 'check', name: 'tests', by: members }, value));
	}

	function update(change: Partial<CompletionRule>) {
		onchange({ ...value, ...change } as CompletionRule);
	}
</script>

<fieldset class="question">
	<legend>{legend}</legend>
	<label class="choice">
		<input type="radio" {name} checked={answer.kind === 'self'} onchange={() => pick('self')} />
		<span>
			<span class="choice-title">The person who did it says so</span>
			<span class="help">Nobody else has to check it.</span>
		</span>
	</label>
	<label class="choice">
		<input type="radio" {name} checked={answer.kind === 'review'} onchange={() => pick('review')} />
		<span>
			<span class="choice-title">Someone reviews it</span>
			<span class="help">One or more people have to approve the work.</span>
		</span>
	</label>
	{#if value.kind === 'reviews'}
		<div class="nested">
			<WhoPicker
				label="Who can review?"
				value={value.by}
				{roles}
				onchange={(by) => update({ by })}
			/>
			<label class="field">
				<span class="label">How many people have to approve?</span>
				<input
					type="number"
					min="1"
					max="99"
					value={value.count}
					onchange={(event) => {
						const count = Number((event.target as HTMLInputElement).value);
						if (Number.isInteger(count) && count >= 0) update({ count });
					}}
				/>
			</label>
			<label class="check">
				<input
					type="checkbox"
					checked={value.exclude_author}
					onchange={(event) =>
						update({ exclude_author: (event.target as HTMLInputElement).checked })}
				/>
				<span>The author can't approve their own work</span>
			</label>
		</div>
	{/if}
	<label class="choice">
		<input type="radio" {name} checked={answer.kind === 'check'} onchange={() => pick('check')} />
		<span>
			<span class="choice-title">An automated check passes</span>
			<span class="help">A named check, such as the tests, is reported as passing.</span>
		</span>
	</label>
	{#if value.kind === 'check'}
		<div class="nested">
			<label class="field">
				<span class="label">Name of the check</span>
				<input
					type="text"
					value={value.name}
					onchange={(event) => update({ name: (event.target as HTMLInputElement).value })}
				/>
			</label>
			<WhoPicker
				label="Who reports the result?"
				value={value.by}
				{roles}
				onchange={(by) => update({ by })}
			/>
			<span class="help">Locust records the report. It can't prove the check really ran.</span>
		</div>
	{/if}
	{#if answer.kind === 'own'}
		<label class="choice">
			<input type="radio" {name} checked disabled />
			<span>
				<span class="choice-title">Your own rule (kept as it is)</span>
				<span class="help">A task is done when {doneClause(value)}.</span>
			</span>
		</label>
	{/if}
</fieldset>
