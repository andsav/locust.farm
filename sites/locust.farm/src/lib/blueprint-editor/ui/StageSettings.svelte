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
	const ruleName = $props.id();

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
</script>

<div class="settings">
	<label class="field">
		<span class="label">Name</span>
		<input type="text" value={name} onchange={rename} />
	</label>

	<fieldset class="question">
		<legend>Starts when</legend>
		{#if stage.requires.length === 0}
			<p class="help">This stage can start any time.</p>
		{/if}
		<ul class="items">
			{#each stage.requires as requirement, index (index)}
				<li class="item row">
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
						class="quiet"
						onclick={() => onchange(removeRequirement(document, name, index))}
					>
						Remove
					</button>
				</li>
			{/each}
		</ul>
		{#if others.length > 0}
			<button type="button" onclick={addPrerequisite}>Add a condition</button>
		{/if}
	</fieldset>

	<WhoPicker
		label="Who gets this step's work?"
		value={stage.recipients}
		{roles}
		allowNobody
		onchange={(recipients) => onchange(setStage(document, name, { recipients }))}
	/>

	<DeciderPicker
		label="Run by"
		value={stage.runner}
		{roles}
		onchange={(runner) => {
			if (runner) onchange(setStage(document, name, { runner }));
		}}
	/>
	<p class="help">
		Whose Locust creates this step's task and sends it out when it is ready. Locust never starts an
		agent.
	</p>

	<fieldset class="question">
		<legend>When is this step done?</legend>
		<label class="choice">
			<input
				type="radio"
				name={ruleName}
				checked={!ownRule}
				onchange={() => onchange(setStageCompletion(document, name, null))}
			/>
			<span>
				<span class="choice-title">Same as the rules on the left</span>
			</span>
		</label>
		<label class="choice">
			<input
				type="radio"
				name={ruleName}
				checked={ownRule}
				onchange={() =>
					onchange(
						setStageCompletion(document, name, {
							kind: 'reviews',
							by: { kind: 'members' },
							count: 1,
							exclude_author: true
						})
					)}
			/>
			<span>
				<span class="choice-title">Its own rule</span>
			</span>
		</label>
	</fieldset>
	{#if ownRule && stage.task_type !== null && Object.hasOwn(blueprint.task_types, stage.task_type)}
		<DonePicker
			legend="This step is done when"
			value={(blueprint.task_types[stage.task_type].decisions ?? blueprint.decisions).completion}
			{roles}
			onchange={(completion) => onchange(setStageCompletion(document, name, completion))}
		/>
	{/if}
</div>
