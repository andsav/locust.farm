<!-- The less common rules, shown in the side panel under "More". -->
<script lang="ts">
	import type { CompletionRule } from '../contract/types.ts';
	import type { EditorDocument } from '../model/document.ts';
	import {
		renameTaskType,
		setDecisions,
		setGuidance,
		setInput,
		setTaskType,
		setWork
	} from '../model/edit.ts';
	import DeciderPicker from './DeciderPicker.svelte';
	import DonePicker from './DonePicker.svelte';
	import Icon from './Icon.svelte';
	import { tip } from './tooltip.ts';
	import WhoPicker from './WhoPicker.svelte';

	let {
		document,
		onchange
	}: {
		document: EditorDocument;
		onchange: (next: EditorDocument) => void;
	} = $props();

	const blueprint = $derived(document.blueprint);
	const roles = $derived(Object.keys(blueprint.roles));
	const stageTaskTypes = $derived(
		new Set(
			Object.values(blueprint.flow).flatMap((stage) => (stage.task_type ? [stage.task_type] : []))
		)
	);

	let newInput = $state('');
	let newTaskType = $state('');

	function setTaskTypeCompletion(name: string, completion: CompletionRule) {
		const current = blueprint.task_types[name];
		onchange(
			setTaskType(document, name, {
				work: current.work,
				decisions: { ...(current.decisions ?? blueprint.decisions), completion }
			})
		);
	}
</script>

<div class="more-settings">
	<section id="section-sharing" aria-labelledby="sharing-title">
		<h3 id="sharing-title">Sharing</h3>
		<WhoPicker
			label="Who can suggest tasks?"
			icon="list-plus"
			text="Suggest tasks"
			value={blueprint.work.propose}
			{roles}
			allowNobody
			onchange={(propose) => onchange(setWork(document, { propose }))}
		/>
		<WhoPicker
			label="Who can share findings?"
			icon="broadcast"
			text="Share findings"
			value={blueprint.work.publish}
			{roles}
			allowNobody
			onchange={(publish) => onchange(setWork(document, { publish }))}
		/>
	</section>

	<section aria-labelledby="deciders-title">
		<h3 id="deciders-title">Decisions</h3>
		<div id="section-final-answer">
			<DeciderPicker
				label="Who picks one final answer?"
				icon="trophy"
				text="Final answer"
				value={blueprint.decisions.selection}
				{roles}
				noneLabel="No final answer"
				onchange={(selection) => onchange(setDecisions(document, { selection }))}
			/>
		</div>
		<div id="section-finish">
			<DeciderPicker
				label="Who can say the whole goal is finished?"
				icon="flag-checkered"
				text="Finish goal"
				value={blueprint.decisions.finish}
				{roles}
				noneLabel="Nobody, stays open"
				onchange={(finish) => onchange(setDecisions(document, { finish }))}
			/>
		</div>
	</section>

	<section id="section-advice" aria-labelledby="advice-title">
		<h3
			id="advice-title"
			use:tip={{
				label: 'Advice for everyone',
				description: 'Shown to people and agents in the goal. It is advice, never a permission.'
			}}
		>
			<Icon name="lightbulb" size={14} /> Advice
		</h3>
		<textarea
			rows="3"
			aria-label="Advice for everyone"
			placeholder="Keep findings short and link sources."
			value={blueprint.context.guidance}
			onchange={(event) =>
				onchange(setGuidance(document, (event.target as HTMLTextAreaElement).value))}></textarea>
	</section>

	<section id="section-material" aria-labelledby="material-title">
		<h3
			id="material-title"
			use:tip={{
				label: 'Starting material',
				description: 'Text or files that whoever starts a goal provides, such as a brief.'
			}}
		>
			<Icon name="file-text" size={14} /> Starting material
		</h3>
		<ul class="items">
			{#each Object.entries(blueprint.context.inputs) as [name, input] (name)}
				<li class="item row">
					<span class="item-name">{name}</span>
					<select
						aria-label={`Kind of "${name}"`}
						value={input.kind}
						onchange={(event) =>
							onchange(
								setInput(document, name, {
									...input,
									kind: (event.target as HTMLSelectElement).value as 'text' | 'artifact'
								})
							)}
					>
						<option value="text">Text</option>
						<option value="artifact">A file</option>
					</select>
					<label class="check" use:tip={'Required when a goal starts'}>
						<input
							type="checkbox"
							checked={input.required}
							onchange={(event) =>
								onchange(
									setInput(document, name, {
										...input,
										required: (event.target as HTMLInputElement).checked
									})
								)}
						/>
						<span>Required</span>
					</label>
					<button
						type="button"
						class="icon-button"
						aria-label={`Remove "${name}"`}
						use:tip={'Remove'}
						onclick={() => onchange(setInput(document, name, null))}
					>
						<Icon name="trash" size={16} />
					</button>
				</li>
			{/each}
		</ul>
		<form
			class="add"
			onsubmit={(event) => {
				event.preventDefault();
				const name = newInput.trim();
				if (name === '' || Object.hasOwn(blueprint.context.inputs, name)) return;
				onchange(setInput(document, name, { kind: 'text', required: true }));
				newInput = '';
			}}
		>
			<input
				type="text"
				bind:value={newInput}
				aria-label="New starting material"
				placeholder="brief"
			/>
			<button
				type="submit"
				class="icon-button"
				aria-label="Add starting material"
				use:tip={'Add'}
				disabled={newInput.trim() === ''}
			>
				<Icon name="plus" size={16} />
			</button>
		</form>
	</section>

	<section id="section-task-types" aria-labelledby="task-types-title">
		<h3
			id="task-types-title"
			use:tip={{
				label: 'Task types',
				description:
					'Named sets of rules. A stage or a task can follow one instead of the main rules.'
			}}
		>
			<Icon name="stack" size={14} /> Task types
		</h3>
		<ul class="items">
			{#each Object.keys(blueprint.task_types) as name (name)}
				<li class="item">
					<div class="add">
						<input
							type="text"
							aria-label="Task type name"
							value={name}
							onchange={(event) => {
								const to = (event.target as HTMLInputElement).value.trim();
								if (to !== '' && to !== name) onchange(renameTaskType(document, name, to));
							}}
						/>
						<button
							type="button"
							class="icon-button"
							aria-label={`Remove "${name}"`}
							use:tip={stageTaskTypes.has(name) ? 'Remove (a stage uses it)' : 'Remove'}
							onclick={() => onchange(setTaskType(document, name, null))}
						>
							<Icon name="trash" size={16} />
						</button>
					</div>
					<DonePicker
						label={`When is a "${name}" task done?`}
						value={(blueprint.task_types[name].decisions ?? blueprint.decisions).completion}
						{roles}
						onchange={(completion) => setTaskTypeCompletion(name, completion)}
					/>
				</li>
			{/each}
		</ul>
		<form
			class="add"
			onsubmit={(event) => {
				event.preventDefault();
				const name = newTaskType.trim();
				if (name === '' || Object.hasOwn(blueprint.task_types, name)) return;
				onchange(setTaskType(document, name, { work: null, decisions: null }));
				newTaskType = '';
			}}
		>
			<input
				type="text"
				bind:value={newTaskType}
				aria-label="New task type"
				placeholder="benchmark"
			/>
			<button
				type="submit"
				class="icon-button"
				aria-label="Add a task type"
				use:tip={'Add'}
				disabled={newTaskType.trim() === ''}
			>
				<Icon name="plus" size={16} />
			</button>
		</form>
	</section>

	<details class="limits">
		<summary><Icon name="info" size={14} /> Things Locust can't do</summary>
		<ul>
			<li>The first person to grab a task keeps it. Hand the task out instead.</li>
			<li>Approved if nobody objects. Ask for one review instead.</li>
			<li>Majority votes. Ask for a fixed number of approvals instead.</li>
			<li>Moving work on a timer. Stages wait for results, never for time.</li>
			<li>Private roles inside a goal. Use a separate goal for a smaller group.</li>
		</ul>
	</details>
</div>

<style>
	.more-settings {
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

	.limits summary {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		color: var(--color-text-subtle);
		cursor: pointer;
	}

	.limits ul {
		display: grid;
		gap: 0.375rem;
		margin: 0.75rem 0 0;
		padding-left: 1rem;
		color: var(--color-text-muted);
	}
</style>
