<!-- The settings of one stage, shown in the side panel. -->
<script lang="ts">
	import type { EvidenceKind } from '../contract/types.ts';
	import type { EditorDocument } from '../model/document.ts';
	import {
		addRequirement,
		removeRequirement,
		renameStage,
		setRequirement,
		setStage,
		setStageCompletion,
		stageOrder,
		wouldLoop
	} from '../model/edit.ts';
	import { EVIDENCE_ORDER, EVIDENCE_WORDS } from '../model/words.ts';
	import DeciderPicker from './DeciderPicker.svelte';
	import DonePicker from './DonePicker.svelte';
	import Icon from './Icon.svelte';
	import Segmented from './Segmented.svelte';
	import { tip } from './tooltip.ts';
	import WhoPicker from './WhoPicker.svelte';

	let {
		document,
		name,
		onchange,
		onrename,
		onannounce
	}: {
		document: EditorDocument;
		name: string;
		onchange: (next: EditorDocument) => void;
		onrename: (to: string) => void;
		onannounce: (message: string) => void;
	} = $props();

	const blueprint = $derived(document.blueprint);
	const stage = $derived(blueprint.flow[name]);
	const roles = $derived(Object.keys(blueprint.roles));
	const others = $derived(stageOrder(document).filter((other) => other !== name));
	const ownRule = $derived(stage.task_type !== null);

	function rename(event: Event) {
		const input = event.target as HTMLInputElement;
		const to = input.value.trim();
		if (to === name) return;
		if (to === '' || Object.hasOwn(blueprint.flow, to)) {
			input.value = name;
			onannounce(to === '' ? 'A stage needs a name.' : `There is already a stage called "${to}".`);
			return;
		}
		onchange(renameStage(document, name, to));
		onrename(to);
	}

	function addPrerequisite() {
		const candidate = others.find((other) => !wouldLoop(blueprint, other, name));
		if (!candidate) {
			onannounce(
				'Every other stage already waits for this one, so this stage cannot wait for them.'
			);
			return;
		}
		onchange(addRequirement(document, candidate, name, 'completion'));
	}

	function pickRule(kind: 'same' | 'own') {
		if (kind === 'same') onchange(setStageCompletion(document, name, null));
		else if (!ownRule) {
			onchange(
				setStageCompletion(document, name, {
					kind: 'reviews',
					by: { kind: 'members' },
					count: 1,
					exclude_author: true
				})
			);
		}
	}
</script>

<div class="settings">
	<label class="field">
		<span class="label">Name</span>
		<input type="text" value={name} onchange={rename} />
	</label>

	<section aria-labelledby="starts-title">
		<h3
			id="starts-title"
			use:tip={{
				label: 'Starts after',
				description: 'This stage waits for these. With none, it can start any time.'
			}}
		>
			<Icon name="flow-arrow" size={14} /> Starts after
		</h3>
		{#if stage.requires.length === 0}
			<p class="faint">Any time</p>
		{/if}
		<ul class="items">
			{#each stage.requires as requirement, index (index)}
				<li class="requirement">
					<select
						aria-label="Stage it waits for"
						value={requirement.stage}
						onchange={(event) => {
							const value = (event.target as HTMLSelectElement).value;
							if (wouldLoop(blueprint, value, name)) {
								onannounce(`That would make "${value}" wait for itself.`);
								(event.target as HTMLSelectElement).value = requirement.stage;
								return;
							}
							onchange(setRequirement(document, name, index, { stage: value }));
						}}
					>
						{#each others as other (other)}
							<option value={other}>{other}</option>
						{/each}
						{#if !others.includes(requirement.stage)}
							<option value={requirement.stage}>{requirement.stage} (not a stage)</option>
						{/if}
					</select>
					<select
						aria-label="What it waits for"
						value={requirement.evidence}
						onchange={(event) =>
							onchange(
								setRequirement(document, name, index, {
									evidence: (event.target as HTMLSelectElement).value as EvidenceKind
								})
							)}
					>
						{#each EVIDENCE_ORDER as evidence (evidence)}
							<option value={evidence}>{EVIDENCE_WORDS[evidence].short}</option>
						{/each}
					</select>
					<button
						type="button"
						class="icon-button"
						aria-label="Remove this condition"
						use:tip={'Remove'}
						onclick={() => onchange(removeRequirement(document, name, index))}
					>
						<Icon name="x" size={16} />
					</button>
				</li>
			{/each}
		</ul>
		{#if others.length > 0}
			<button
				type="button"
				class="icon-button"
				aria-label="Add a condition"
				use:tip={'Wait for another stage'}
				onclick={addPrerequisite}
			>
				<Icon name="plus" size={16} />
			</button>
		{/if}
	</section>

	<section aria-labelledby="who-title">
		<h3 id="who-title">People</h3>
		<WhoPicker
			label="Who gets this stage's work?"
			icon="tray-arrow-down"
			text="Work goes to"
			value={stage.recipients}
			{roles}
			allowNobody
			onchange={(recipients) => onchange(setStage(document, name, { recipients }))}
		/>
		<DeciderPicker
			label="Run by"
			icon="play-circle"
			text="Run by"
			description="Whose Locust creates this stage's task and sends it out when it is ready. Locust never starts an agent."
			value={stage.runner}
			{roles}
			onchange={(runner) => {
				if (runner) onchange(setStage(document, name, { runner }));
			}}
		/>
	</section>

	<section aria-labelledby="stage-done-title">
		<h3 id="stage-done-title" use:tip={'When is this stage done?'}>
			<Icon name="seal-check" size={14} /> Done
		</h3>
		<Segmented
			label="When is this stage done?"
			value={ownRule ? 'own' : 'same'}
			options={[
				{
					value: 'same',
					icon: 'seal-check',
					text: 'Main rule',
					tip: 'The same as the Done rule beside the map.'
				},
				{
					value: 'own',
					icon: 'stack',
					text: 'Own rule',
					tip: 'This stage has its own rule, such as needing a review.'
				}
			]}
			onchange={pickRule}
		/>
		{#if ownRule && stage.task_type !== null && Object.hasOwn(blueprint.task_types, stage.task_type)}
			<DonePicker
				label="When is this stage done?"
				value={(blueprint.task_types[stage.task_type].decisions ?? blueprint.decisions).completion}
				{roles}
				onchange={(completion) => onchange(setStageCompletion(document, name, completion))}
			/>
		{/if}
	</section>
</div>

<style>
	.settings {
		display: grid;
		gap: 1.5rem;
	}

	h3 {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		justify-self: start;
		color: var(--color-text-subtle);
		font: var(--text-label);
		letter-spacing: var(--tracking-label);
		text-transform: uppercase;
	}

	.faint {
		color: var(--color-text-faint);
	}

	.requirement {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto;
		gap: 0.375rem;
		align-items: center;
	}

	section > .icon-button {
		justify-self: start;
	}
</style>
