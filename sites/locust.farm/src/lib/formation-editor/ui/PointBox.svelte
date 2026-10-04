<!--
	The choices for one point of a line, shown under it. Every option is a full
	phrase, every field has a word, and the limit that belongs to a setting is
	printed beside it.
-->
<script lang="ts">
	import type { EditorDocument } from '../model/document.ts';
	import { addRole, stageOrder, wouldLoop } from '../model/edit.ts';
	import {
		addAnswer,
		afterAnswer,
		countsAnswer,
		follows,
		lineRules,
		noTasks,
		pickAnswer,
		setAdd,
		setAfter,
		setCounts,
		setPick,
		setWork,
		useMain,
		workAnswer,
		type Approvals,
		type Check,
		type LineRef,
		type PointName,
		type Who
	} from '../model/line.ts';
	import { phrase, QUESTIONS } from '../model/words.ts';
	import Icon from './Icon.svelte';
	import WhoSelect from './WhoSelect.svelte';

	let {
		document,
		line,
		point,
		onchange,
		onclose
	}: {
		document: EditorDocument;
		line: LineRef;
		point: PointName;
		onchange: (next: EditorDocument) => void;
		onclose: () => void;
	} = $props();

	const id = $props.id();
	const formation = $derived(document.formation);
	const roles = $derived(Object.keys(formation.roles));
	const rules = $derived(lineRules(formation, line));
	const stepAdd = $derived(line.kind === 'step' && point === 'add');
	const title = $derived(stepAdd ? 'When does Locust add this task?' : QUESTIONS[point]);
	const same = $derived(line.kind !== 'main' && !stepAdd && follows(document, line, point));
	const hasSteps = $derived(Object.keys(formation.flow).length > 0);

	/** An option that was chosen but still needs a role to be named. */
	let waiting = $state<string | null>(null);

	type Change = (document: EditorDocument, name: string) => EditorDocument;

	/** Picks an option that needs a role: the first role, or a name asked for in place. */
	function needsRole(option: string, apply: Change) {
		if (roles.length > 0) {
			waiting = null;
			onchange(apply(document, roles[0]));
		} else {
			waiting = option;
		}
	}

	function withNewRole(name: string, apply: Change) {
		waiting = null;
		onchange(apply(addRole(document, name), name));
	}

	// Who adds tasks.
	const add = $derived(addAnswer(rules.work));
	const addRoleChange: Change = (d, name) => setAdd(d, line, { kind: 'role', name });

	// When Locust adds a step's task.
	const after = $derived(line.kind === 'step' ? afterAnswer(formation, line.name) : null);
	const earlier = $derived(
		line.kind === 'step'
			? stageOrder(formation).filter(
					(other) => other !== line.name && !wouldLoop(formation, other, line.name)
				)
			: []
	);

	// Who works on a task.
	const work = $derived(workAnswer(rules.work));
	const idle = $derived(noTasks(formation, line));
	const workRoleChange: Change = (d, name) => setWork(d, line, { kind: 'role', name });
	const asksChange = (d: EditorDocument, by: Who) => setWork(d, line, { kind: 'asks', by });

	// When a result counts.
	const counts = $derived(countsAnswer(rules.decisions.completion));
	const list = $derived(
		counts.kind === 'list' ? counts : { kind: 'list' as const, approvals: null, check: null }
	);
	const setApprovals = (d: EditorDocument, approvals: Approvals | null) =>
		setCounts(d, line, { ...list, approvals });
	const setCheck = (d: EditorDocument, check: Check | null) =>
		setCounts(d, line, { ...list, check });

	// Whether one result is picked, and who can close a task.
	const pick = $derived(pickAnswer(rules.decisions));
	const pickChange: Change = (d, name) => setPick(d, line, { pick: { kind: 'role', name } });
	const closeChange: Change = (d, name) => setPick(d, line, { close: { kind: 'role', name } });

	// A waiting option only shows while there is still no role to pick.
	const chosen = (answer: string) => (waiting !== null && roles.length === 0 ? waiting : answer);
</script>

<div class="box" role="group" aria-labelledby={`${id}-title`}>
	<header>
		<h3 id={`${id}-title`}>{title}</h3>
		{#if line.kind !== 'main' && !stepAdd}
			{#if same}
				<p class="muted">Same as any task. Change it here to give this one its own answer.</p>
			{:else}
				<button
					type="button"
					class="quiet"
					onclick={() => onchange(useMain(document, line, point))}
				>
					Use the same as any task
				</button>
			{/if}
		{/if}
		<button type="button" class="icon-button close" aria-label="Close" onclick={onclose}>
			<Icon name="x" size={16} />
		</button>
	</header>

	{#if stepAdd && line.kind === 'step' && after}
		<div class="options">
			<label class="option">
				<input
					type="radio"
					name={`${id}-after`}
					checked={after.kind === 'start'}
					onchange={() => onchange(setAfter(document, line.name, null))}
				/>
				<span>At the start</span>
			</label>
			<label class="option" class:disabled={earlier.length === 0}>
				<input
					type="radio"
					name={`${id}-after`}
					checked={after.kind === 'after'}
					disabled={earlier.length === 0}
					onchange={() => onchange(setAfter(document, line.name, earlier[0]))}
				/>
				<span>After another step has a result that counts</span>
			</label>
			{#if after.kind === 'after'}
				<div class="fields">
					<label class="field">
						<span class="label">Which step?</span>
						<select
							value={after.step}
							onchange={(event) =>
								onchange(setAfter(document, line.name, (event.target as HTMLSelectElement).value))}
						>
							{#each earlier as other (other)}
								<option value={other}>{other}</option>
							{/each}
							{#if !earlier.includes(after.step)}
								<option value={after.step}>{after.step}</option>
							{/if}
						</select>
					</label>
					{#if after.picked}
						<p class="note">Someone picks a result in that step, so this one waits for the pick.</p>
					{/if}
				</div>
			{/if}
			{#if after.kind === 'own'}
				<p class="note">
					Kept as it is: {phrase(formation, line, 'add')} Choosing an option replaces it.
				</p>
			{/if}
		</div>
		<p class="note">
			The Locust of whoever started the goal adds the task, so that computer has to be on.
		</p>
	{:else if point === 'add'}
		<div class="options">
			<label class="option">
				<input
					type="radio"
					name={`${id}-add`}
					checked={chosen(add.kind) === 'anyone'}
					onchange={() => {
						waiting = null;
						onchange(setAdd(document, line, { kind: 'anyone' }));
					}}
				/>
				<span>Anyone</span>
			</label>
			<label class="option">
				<input
					type="radio"
					name={`${id}-add`}
					checked={chosen(add.kind) === 'role'}
					onchange={() => needsRole('role', addRoleChange)}
				/>
				<span>Only one role</span>
			</label>
			{#if chosen(add.kind) === 'role'}
				<div class="fields">
					<WhoSelect
						label="Which role?"
						value={add.kind === 'role' ? add : null}
						{roles}
						onpick={(who) => who.kind === 'role' && onchange(addRoleChange(document, who.name))}
						onnewrole={(name) => withNewRole(name, addRoleChange)}
					/>
				</div>
			{/if}
			<label class="option">
				<input
					type="radio"
					name={`${id}-add`}
					checked={chosen(add.kind) === 'none'}
					onchange={() => {
						waiting = null;
						onchange(setAdd(document, line, { kind: 'none' }));
					}}
				/>
				<span>
					{line.kind === 'kind'
						? 'Nobody'
						: hasSteps
							? 'Nobody. Only Locust adds tasks, as steps.'
							: 'Nobody. There are no tasks; members only post results.'}
				</span>
			</label>
			{#if add.kind === 'own'}
				<p class="note">
					Kept as it is: {phrase(formation, line, 'add')} Choosing an option replaces it.
				</p>
			{/if}
		</div>
	{:else if point === 'work'}
		<div class="options">
			<label class="option" class:disabled={idle}>
				<input
					type="radio"
					name={`${id}-work`}
					disabled={idle}
					checked={chosen(work.kind) === 'anyone'}
					onchange={() => {
						waiting = null;
						onchange(setWork(document, line, { kind: 'anyone' }));
					}}
				/>
				<span>Anyone</span>
				<span class="note">No lock: two members can work on the same task.</span>
			</label>
			<label class="option" class:disabled={idle}>
				<input
					type="radio"
					name={`${id}-work`}
					disabled={idle}
					checked={chosen(work.kind) === 'role'}
					onchange={() => needsRole('role', workRoleChange)}
				/>
				<span>Only one role</span>
				<span class="note">Only they can post results.</span>
			</label>
			{#if chosen(work.kind) === 'role'}
				<div class="fields">
					<WhoSelect
						label="Which role?"
						value={work.kind === 'role' ? work : null}
						{roles}
						onpick={(who) => who.kind === 'role' && onchange(workRoleChange(document, who.name))}
						onnewrole={(name) => withNewRole(name, workRoleChange)}
					/>
				</div>
			{/if}
			<label class="option" class:disabled={idle}>
				<input
					type="radio"
					name={`${id}-work`}
					disabled={idle}
					checked={chosen(work.kind) === 'asks'}
					onchange={() => {
						waiting = null;
						onchange(
							asksChange(
								document,
								roles.length > 0 ? { kind: 'role', name: roles[0] } : { kind: 'anyone' }
							)
						);
					}}
				/>
				<span>A member is asked to do it</span>
				<span class="note">The member asked can say no. Other members can still post results.</span>
			</label>
			{#if work.kind === 'asks'}
				<div class="fields">
					<WhoSelect
						label="Who asks?"
						value={work.by}
						{roles}
						anyone="Any member"
						onpick={(who) => who.kind !== 'nobody' && onchange(asksChange(document, who))}
						onnewrole={(name) => withNewRole(name, (d) => asksChange(d, { kind: 'role', name }))}
					/>
				</div>
			{/if}
			{#if idle}
				<p class="note">There are no tasks to work on. Change who adds tasks first.</p>
			{:else if work.kind === 'none'}
				<p class="note">
					Now: nobody starts work on a task, and members post results directly. Choosing an option
					replaces it.
				</p>
			{:else if work.kind === 'own'}
				<p class="note">
					Kept as it is: {phrase(formation, line, 'work')} Choosing an option replaces it.
				</p>
			{/if}
		</div>
	{:else if point === 'counts'}
		<div class="options">
			<label class="option">
				<input
					type="checkbox"
					checked={list.approvals !== null}
					onchange={(event) =>
						onchange(
							setApprovals(
								document,
								(event.target as HTMLInputElement).checked
									? { by: { kind: 'anyone' }, count: 1, excludeAuthor: true }
									: null
							)
						)}
				/>
				<span>It has approvals</span>
			</label>
			{#if list.approvals !== null}
				{@const approvals = list.approvals}
				<div class="fields">
					<label class="field narrow">
						<span class="label">How many?</span>
						<input
							type="number"
							min="1"
							max="99"
							value={approvals.count}
							onchange={(event) => {
								const input = event.target as HTMLInputElement;
								const count = Number(input.value);
								if (Number.isInteger(count) && count >= 1 && count <= 99) {
									onchange(setApprovals(document, { ...approvals, count }));
								} else {
									input.value = String(approvals.count);
								}
							}}
						/>
					</label>
					<WhoSelect
						label="From whom?"
						value={approvals.by}
						{roles}
						anyone="Anyone"
						onpick={(who) =>
							who.kind !== 'nobody' && onchange(setApprovals(document, { ...approvals, by: who }))}
						onnewrole={(name) =>
							withNewRole(name, (d) =>
								setApprovals(d, { ...approvals, by: { kind: 'role', name } })
							)}
					/>
					<label class="check">
						<input
							type="checkbox"
							checked={approvals.excludeAuthor}
							onchange={(event) =>
								onchange(
									setApprovals(document, {
										...approvals,
										excludeAuthor: (event.target as HTMLInputElement).checked
									})
								)}
						/>
						<span>The author's own approval does not count</span>
					</label>
				</div>
			{/if}
			<label class="option">
				<input
					type="checkbox"
					checked={list.check !== null}
					onchange={(event) =>
						onchange(
							setCheck(
								document,
								(event.target as HTMLInputElement).checked
									? { name: 'tests', by: { kind: 'anyone' } }
									: null
							)
						)}
				/>
				<span>A check is reported as passed</span>
				<span class="note">
					Locust does not run the check. A member, who can be the author, reports that it passed.
				</span>
			</label>
			{#if list.check !== null}
				{@const check = list.check}
				<div class="fields">
					<label class="field">
						<span class="label">Name of the check</span>
						<input
							type="text"
							value={check.name}
							onchange={(event) => {
								const input = event.target as HTMLInputElement;
								const name = input.value.trim();
								if (name === '') input.value = check.name;
								else onchange(setCheck(document, { ...check, name }));
							}}
						/>
					</label>
					<WhoSelect
						label="Who reports it?"
						value={check.by}
						{roles}
						anyone="Anyone"
						onpick={(who) =>
							who.kind !== 'nobody' && onchange(setCheck(document, { ...check, by: who }))}
						onnewrole={(name) =>
							withNewRole(name, (d) => setCheck(d, { ...check, by: { kind: 'role', name } }))}
					/>
				</div>
			{/if}
			{#if counts.kind === 'own'}
				<p class="note">
					Kept as it is: {phrase(formation, line, 'counts')} Ticking a box replaces it.
				</p>
			{:else if list.approvals === null && list.check === null}
				<p class="note">With nothing ticked, a result counts when its author says so.</p>
			{:else}
				<p class="note">
					A result counts as soon as it has everything ticked. A rejection, or a report that the
					check failed, is recorded and does not take that away.
				</p>
			{/if}
		</div>
	{:else}
		<div class="options">
			<label class="option">
				<input
					type="radio"
					name={`${id}-pick`}
					checked={chosen(pick.pick.kind) === 'nobody'}
					onchange={() => {
						waiting = null;
						onchange(setPick(document, line, { pick: { kind: 'nobody' } }));
					}}
				/>
				<span>Nobody</span>
				<span class="note">Every result that counts is kept. None is marked as the one to use.</span
				>
			</label>
			<label class="option">
				<input
					type="radio"
					name={`${id}-pick`}
					checked={chosen(pick.pick.kind) === 'role'}
					onchange={() => needsRole('role', pickChange)}
				/>
				<span>One role picks one result per task</span>
				<span class="note">
					The picked result is the one to use; the others are kept. The role must have exactly one
					member.
				</span>
			</label>
			{#if chosen(pick.pick.kind) === 'role'}
				<div class="fields">
					<WhoSelect
						label="Which role?"
						value={pick.pick.kind === 'role' ? pick.pick : null}
						{roles}
						onpick={(who) => who.kind === 'role' && onchange(pickChange(document, who.name))}
						onnewrole={(name) => withNewRole(name, pickChange)}
					/>
				</div>
			{/if}
			{#if pick.pick.kind === 'own'}
				<p class="note">
					Kept as it is: {phrase(formation, line, 'pick')} Choosing an option replaces it.
				</p>
			{/if}
		</div>
		<div class="fields apart">
			<WhoSelect
				label="Who can close a task?"
				value={pick.close.kind === 'own' ? null : pick.close}
				{roles}
				nobody="Nobody"
				kept={pick.close.kind === 'own' ? 'One specific member' : null}
				onpick={(who) =>
					onchange(
						who.kind === 'role'
							? closeChange(document, who.name)
							: setPick(document, line, { close: { kind: 'nobody' } })
					)}
				onnewrole={(name) => withNewRole(name, closeChange)}
			/>
			<p class="note">
				Closing a task stops new work on it. The role must have exactly one member.
			</p>
		</div>
	{/if}
</div>

<style>
	.box {
		display: grid;
		gap: 0.875rem;
		padding: 1rem 1.25rem 1.25rem;
		border-top: 1px solid var(--color-accent);
		background: var(--color-surface);
	}

	header {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem 1rem;
		align-items: center;
	}

	h3 {
		flex: 1;
		min-width: 12rem;
	}

	.close {
		border-color: transparent;
		background: transparent;
	}

	.options {
		display: grid;
		gap: 0.5rem;
		max-width: 44rem;
	}

	.option {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 0 0.625rem;
		align-items: baseline;
		cursor: pointer;
	}

	.option input {
		accent-color: var(--color-accent);
	}

	.option .note {
		grid-column: 2;
	}

	.option.disabled {
		color: var(--color-text-faint);
		cursor: not-allowed;
	}

	.fields {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem 1rem;
		align-items: end;
		margin: 0.125rem 0 0.375rem 1.625rem;
	}

	.fields > :global(.who),
	.fields > .field {
		flex: 0 1 16rem;
	}

	.fields > .field.narrow {
		flex: 0 0 6rem;
	}

	.fields > .check {
		min-height: 2.25rem;
	}

	.fields.apart {
		margin-left: 0;
		padding-top: 0.875rem;
		border-top: var(--border-hairline);
	}

	.fields .note {
		flex: 1 1 100%;
	}

	.note {
		color: var(--color-text-subtle);
	}

	.muted {
		margin: 0;
	}
</style>
