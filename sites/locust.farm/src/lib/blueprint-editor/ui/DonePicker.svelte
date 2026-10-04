<!-- "When is a task done?" as three icon choices, with the details of the chosen one underneath. -->
<script lang="ts">
	import type { CompletionRule, Selector } from '../contract/types.ts';
	import { doneAnswer, doneRule, type DoneAnswer } from '../model/answers.ts';
	import { doneClause } from '../model/words.ts';
	import Icon from './Icon.svelte';
	import Segmented, { type Option } from './Segmented.svelte';
	import { tip } from './tooltip.ts';
	import WhoPicker from './WhoPicker.svelte';

	let {
		value,
		roles,
		label,
		onchange
	}: {
		value: CompletionRule;
		roles: string[];
		/** The question, read by screen readers and shown in the tooltip. */
		label: string;
		onchange: (value: CompletionRule) => void;
	} = $props();

	const answer = $derived(doneAnswer(value));

	const options = $derived<Option<DoneAnswer['kind']>[]>([
		{
			value: 'self',
			icon: 'user-check',
			text: 'Says so',
			tip: 'The person who did it says it is done. Nobody else has to check it.'
		},
		{
			value: 'review',
			icon: 'thumbs-up',
			text: 'Review',
			tip: 'One or more people have to approve the work.'
		},
		{
			value: 'check',
			icon: 'test-tube',
			text: 'Check',
			tip: 'A named automated check, such as the tests, is reported as passing.'
		},
		...(answer.kind === 'own'
			? [
					{
						value: 'own' as const,
						icon: 'stack' as const,
						text: 'Own',
						tip: `Your own rule, kept as it is: a task is done when ${doneClause(value)}.`,
						disabled: true
					}
				]
			: [])
	]);

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

<div class="done">
	<Segmented {label} {options} value={answer.kind} onchange={pick} />
	{#if value.kind === 'reviews'}
		<div class="details">
			<WhoPicker
				label="Who can review?"
				icon="users"
				value={value.by}
				{roles}
				onchange={(by) => update({ by })}
			/>
			<div class="line">
				<label class="count" use:tip={'How many people have to approve'}>
					<Icon name="thumbs-up" size={16} />
					<span class="visually-hidden">How many people have to approve?</span>
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
				<label class="check" use:tip={"The author can't approve their own work"}>
					<input
						type="checkbox"
						checked={value.exclude_author}
						onchange={(event) =>
							update({ exclude_author: (event.target as HTMLInputElement).checked })}
					/>
					<span>Not the author</span>
				</label>
			</div>
		</div>
	{/if}
	{#if value.kind === 'check'}
		<div class="details">
			<label
				class="field inline"
				use:tip={{
					label: 'Name of the check',
					description: "Locust records the report. It can't prove the check really ran."
				}}
			>
				<Icon name="test-tube" size={16} />
				<span class="visually-hidden">Name of the check</span>
				<input
					type="text"
					value={value.name}
					onchange={(event) => update({ name: (event.target as HTMLInputElement).value })}
				/>
			</label>
			<WhoPicker
				label="Who reports the result?"
				icon="user"
				value={value.by}
				{roles}
				onchange={(by) => update({ by })}
			/>
		</div>
	{/if}
</div>

<style>
	.done {
		display: grid;
		gap: 0.5rem;
	}

	.details {
		display: grid;
		gap: 0.5rem;
	}

	.line {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		align-items: center;
	}

	.count {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		color: var(--color-text-subtle);
	}

	.count input {
		width: 4.5rem;
	}
</style>
