<!-- The rules on the left: the way of working, then plain questions that adjust it. -->
<script lang="ts">
	import type { CompletionRule, Selector } from '../contract/types.ts';
	import { setStartAnswer, startAnswer } from '../model/answers.ts';
	import type { EditorDocument } from '../model/document.ts';
	import {
		addRole,
		describeRole,
		removeRole,
		renameRole,
		renameTaskType,
		roleUses,
		setDecisions,
		setGuidance,
		setInput,
		setTaskType,
		setWork
	} from '../model/edit.ts';
	import { wayOfWorking } from '../model/presets.ts';
	import { startSentence } from '../model/words.ts';
	import DeciderPicker from './DeciderPicker.svelte';
	import DonePicker from './DonePicker.svelte';
	import WhoPicker from './WhoPicker.svelte';

	let {
		document,
		changed,
		onchange,
		onchangeway,
		onannounce
	}: {
		document: EditorDocument;
		changed: string[];
		onchange: (next: EditorDocument) => void;
		onchangeway: () => void;
		onannounce: (message: string) => void;
	} = $props();

	const blueprint = $derived(document.blueprint);
	const roles = $derived(Object.keys(blueprint.roles));
	const way = $derived(wayOfWorking(document.way));
	const start = $derived(startAnswer(blueprint.work.starts));
	const startName = $props.id();
	const moreChanged = $derived(
		changed.filter((area) =>
			['final answer', 'finish', 'advice', 'starting material', 'task types'].includes(area)
		).length
	);
	const stageTaskTypes = $derived(
		new Set(
			Object.values(blueprint.flow).flatMap((stage) => (stage.task_type ? [stage.task_type] : []))
		)
	);

	let newRole = $state('');
	let newInput = $state('');
	let newTaskType = $state('');

	function addRoleNamed() {
		const name = newRole.trim();
		if (name === '' || Object.hasOwn(blueprint.roles, name)) return;
		onchange(addRole(document, name));
		onannounce(`Added the role "${name}".`);
		newRole = '';
	}

	function rename(from: string, event: Event) {
		const input = event.target as HTMLInputElement;
		const to = input.value.trim();
		if (to === from) return;
		if (to === '' || Object.hasOwn(blueprint.roles, to)) {
			input.value = from;
			onannounce(to === '' ? 'A role needs a name.' : `There is already a role called "${to}".`);
			return;
		}
		onchange(renameRole(document, from, to));
		onannounce(`Renamed "${from}" to "${to}" everywhere it is used.`);
	}

	function remove(name: string) {
		const uses = roleUses(blueprint, name);
		onchange(removeRole(document, name));
		onannounce(
			uses > 0
				? `Removed "${name}". ${uses} rule${uses === 1 ? '' : 's'} used it and now name nobody; check the problems list. Undo brings it back.`
				: `Removed "${name}".`
		);
	}

	function setStartBy(by: Selector) {
		onchange(setStartAnswer(document, { kind: 'handed-out', by }));
	}

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

<div class="rules">
	<section class="way" aria-labelledby="way-title">
		<p class="eyebrow">How your team works</p>
		<h2 id="way-title">{way ? way.title : 'Your own way of working'}</h2>
		<p class="muted">
			{way
				? way.sentence
				: 'Opened from a blueprint. Change it below, or start from one of the six ways.'}
		</p>
		{#if changed.length > 0 && way}
			<p class="muted">
				{changed.length === 1 ? '1 change' : `${changed.length} changes`} from the {way.title} way.
			</p>
		{/if}
		<button type="button" onclick={onchangeway}>Change the way of working</button>
	</section>

	<section id="section-roles" aria-labelledby="roles-title">
		<h3 id="roles-title">Who is involved?</h3>
		<p class="help">
			Everyone in the goal can take part. Roles are jobs, such as a reviewer or a coordinator. You
			name the roles here and choose who fills them when you start a goal.
		</p>
		{#if roles.length === 0}
			<p class="muted">No roles. Everyone in the goal takes part on equal terms.</p>
		{/if}
		<ul class="items">
			{#each roles as role (role)}
				<li class="item">
					<label class="field">
						<span class="label">Role name</span>
						<input type="text" value={role} onchange={(event) => rename(role, event)} />
					</label>
					<label class="field">
						<span class="label">What this role does (optional)</span>
						<input
							type="text"
							value={blueprint.roles[role].description}
							onchange={(event) =>
								onchange(describeRole(document, role, (event.target as HTMLInputElement).value))}
						/>
					</label>
					<button type="button" class="quiet" onclick={() => remove(role)}>Remove "{role}"</button>
				</li>
			{/each}
		</ul>
		<form
			class="add"
			onsubmit={(event) => {
				event.preventDefault();
				addRoleNamed();
			}}
		>
			<label class="field">
				<span class="label">New role</span>
				<input type="text" bind:value={newRole} placeholder="for example: reviewer" />
			</label>
			<button type="submit" disabled={newRole.trim() === ''}>Add a role</button>
		</form>
	</section>

	<section id="section-work" aria-labelledby="work-title">
		<fieldset class="question">
			<legend id="work-title">How does work start?</legend>
			<label class="choice">
				<input
					type="radio"
					name={startName}
					checked={start.kind === 'anyone'}
					onchange={() => onchange(setStartAnswer(document, { kind: 'anyone' }))}
				/>
				<span>
					<span class="choice-title">Anyone can start working on a task</span>
					<span class="help">Several people may work on the same task at once.</span>
				</span>
			</label>
			<label class="choice">
				<input
					type="radio"
					name={startName}
					checked={start.kind === 'handed-out'}
					onchange={() =>
						setStartBy(roles.length > 0 ? { kind: 'role', name: roles[0] } : { kind: 'members' })}
				/>
				<span>
					<span class="choice-title">Someone hands out work</span>
					<span class="help">The person asked has to accept before starting.</span>
				</span>
			</label>
			{#if start.kind === 'handed-out'}
				<div class="nested">
					<WhoPicker
						label="Who hands out work?"
						value={start.by}
						{roles}
						allowTaskCreator
						onchange={setStartBy}
					/>
				</div>
			{/if}
			<label class="choice">
				<input
					type="radio"
					name={startName}
					checked={start.kind === 'nobody'}
					onchange={() => onchange(setStartAnswer(document, { kind: 'nobody' }))}
				/>
				<span>
					<span class="choice-title">Nobody starts tasks</span>
					<span class="help">People only share findings.</span>
				</span>
			</label>
			{#if start.kind === 'own'}
				<label class="choice">
					<input type="radio" name={startName} checked disabled />
					<span>
						<span class="choice-title">Your own setup (kept as it is)</span>
						{#each blueprint.work.starts as rule, index (index)}
							<span class="help">{startSentence(rule)}</span>
						{/each}
					</span>
				</label>
			{/if}
		</fieldset>
		<WhoPicker
			label="Who can suggest tasks?"
			value={blueprint.work.propose}
			{roles}
			allowNobody
			onchange={(propose) => onchange(setWork(document, { propose }))}
		/>
		<WhoPicker
			label="Who can share findings?"
			value={blueprint.work.publish}
			{roles}
			allowNobody
			onchange={(publish) => onchange(setWork(document, { publish }))}
		/>
	</section>

	<section id="section-done">
		<DonePicker
			legend="When is a task done?"
			value={blueprint.decisions.completion}
			{roles}
			onchange={(completion) => onchange(setDecisions(document, { completion }))}
		/>
	</section>

	<details class="more">
		<summary>
			More choices{moreChanged > 0 ? ` (${moreChanged} changed from the way of working)` : ''}
		</summary>

		<section id="section-final-answer">
			<DeciderPicker
				label="Do you need one final answer?"
				value={blueprint.decisions.selection}
				{roles}
				noneLabel="No. Every result that is done is kept."
				onchange={(selection) => onchange(setDecisions(document, { selection }))}
			/>
		</section>

		<section id="section-finish">
			<DeciderPicker
				label="Who can say the whole goal is finished?"
				value={blueprint.decisions.finish}
				{roles}
				noneLabel="Nobody. The goal stays open."
				onchange={(finish) => onchange(setDecisions(document, { finish }))}
			/>
		</section>

		<section id="section-advice">
			<label class="field">
				<span class="label">Advice for everyone</span>
				<textarea
					rows="3"
					value={blueprint.context.guidance}
					onchange={(event) =>
						onchange(setGuidance(document, (event.target as HTMLTextAreaElement).value))}
				></textarea>
				<span class="help"
					>Shown to people and agents in the goal. It is advice, never a permission.</span
				>
			</label>
		</section>

		<section id="section-material" aria-labelledby="material-title">
			<h3 id="material-title">Starting material</h3>
			<p class="help">Text or files that whoever starts a goal provides, such as a brief.</p>
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
						<label class="check">
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
							class="quiet"
							onclick={() => onchange(setInput(document, name, null))}
						>
							Remove
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
				<label class="field">
					<span class="label">New starting material</span>
					<input type="text" bind:value={newInput} placeholder="for example: brief" />
				</label>
				<button type="submit" disabled={newInput.trim() === ''}>Add</button>
			</form>
		</section>

		<section id="section-task-types" aria-labelledby="task-types-title">
			<h3 id="task-types-title">Different rules for some tasks</h3>
			<p class="help">
				Named sets of rules. A stage or a task can follow one instead of the rules above. These are
				called task types.
			</p>
			<ul class="items">
				{#each Object.keys(blueprint.task_types) as name (name)}
					<li class="item">
						<label class="field">
							<span class="label">Task type name</span>
							<input
								type="text"
								value={name}
								onchange={(event) => {
									const to = (event.target as HTMLInputElement).value.trim();
									if (to !== '' && to !== name) onchange(renameTaskType(document, name, to));
								}}
							/>
						</label>
						{#if stageTaskTypes.has(name)}
							<p class="help">Used by a stage.</p>
						{/if}
						<DonePicker
							legend={`When is a "${name}" task done?`}
							value={(blueprint.task_types[name].decisions ?? blueprint.decisions).completion}
							{roles}
							onchange={(completion) => setTaskTypeCompletion(name, completion)}
						/>
						<button
							type="button"
							class="quiet"
							onclick={() => onchange(setTaskType(document, name, null))}
						>
							Remove "{name}"
						</button>
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
				<label class="field">
					<span class="label">New task type</span>
					<input type="text" bind:value={newTaskType} placeholder="for example: benchmark" />
				</label>
				<button type="submit" disabled={newTaskType.trim() === ''}>Add</button>
			</form>
		</section>
	</details>

	<details class="more">
		<summary>Things Locust can't do</summary>
		<ul class="not-possible">
			<li>
				<strong>The first person to grab a task keeps it.</strong> Several people may work on the same
				task; each attempt is kept. Use "Someone hands out work" to give a task to one person.
			</li>
			<li>
				<strong>Approved if nobody objects.</strong> Locust can't see silence across computers that are
				offline. Ask for one review instead.
			</li>
			<li>
				<strong>Majority votes.</strong> Ask for a fixed number of approvals instead, such as two.
			</li>
			<li>
				<strong>Moving work on a timer.</strong> Stages wait for results, never for time.
			</li>
			<li>
				<strong>Private roles inside a goal.</strong> Everyone in a goal can see everything shared in
				it. Use a separate goal for a smaller group.
			</li>
		</ul>
	</details>
</div>
