<!-- The rules beside the map: roles, how work starts and when a task is done. The rest is under More. -->
<script lang="ts">
	import type { Selector } from '../contract/types.ts';
	import { setStartAnswer, startAnswer, type StartAnswer } from '../model/answers.ts';
	import type { EditorDocument } from '../model/document.ts';
	import { addRole, setDecisions } from '../model/edit.ts';
	import { startSentence } from '../model/words.ts';
	import DonePicker from './DonePicker.svelte';
	import Icon from './Icon.svelte';
	import Segmented, { type Option } from './Segmented.svelte';
	import { tip } from './tooltip.ts';
	import WhoPicker from './WhoPicker.svelte';

	let {
		document,
		moreChanged,
		openRole,
		moreOpen,
		onchange,
		onopenrole,
		onopenmore,
		onannounce
	}: {
		document: EditorDocument;
		/** How many of the settings under More differ from the way of working. */
		moreChanged: number;
		/** The role whose settings are open, if any. */
		openRole: string | null;
		moreOpen: boolean;
		onchange: (next: EditorDocument) => void;
		onopenrole: (name: string) => void;
		onopenmore: () => void;
		onannounce: (message: string) => void;
	} = $props();

	const blueprint = $derived(document.blueprint);
	const roles = $derived(Object.keys(blueprint.roles));
	const start = $derived(startAnswer(blueprint.work.starts));

	const startOptions = $derived<Option<StartAnswer['kind']>[]>([
		{
			value: 'anyone',
			icon: 'users',
			text: 'Anyone',
			tip: 'Anyone can start working on a task. Several people may work on the same task at once.'
		},
		{
			value: 'handed-out',
			icon: 'paper-plane-tilt',
			text: 'Handed out',
			tip: 'Someone hands out work. The person asked has to accept before starting.'
		},
		{
			value: 'nobody',
			icon: 'prohibit',
			text: 'Nobody',
			tip: 'Nobody starts tasks. People only share findings.'
		},
		...(start.kind === 'own'
			? [
					{
						value: 'own' as const,
						icon: 'stack' as const,
						text: 'Own',
						tip: `Your own setup, kept as it is. ${blueprint.work.starts.map(startSentence).join(' ')}`,
						disabled: true
					}
				]
			: [])
	]);

	let adding = $state(false);
	let newRole = $state('');

	function addRoleNamed() {
		const name = newRole.trim();
		adding = false;
		newRole = '';
		if (name === '') return;
		if (Object.hasOwn(blueprint.roles, name)) {
			onannounce(`There is already a role called "${name}".`);
			return;
		}
		onchange(addRole(document, name));
		onannounce(`Added the role "${name}".`);
	}

	function setStartBy(by: Selector) {
		onchange(setStartAnswer(document, { kind: 'handed-out', by }));
	}

	function pickStart(kind: StartAnswer['kind']) {
		if (kind === 'anyone') onchange(setStartAnswer(document, { kind: 'anyone' }));
		if (kind === 'nobody') onchange(setStartAnswer(document, { kind: 'nobody' }));
		if (kind === 'handed-out') {
			setStartBy(roles.length > 0 ? { kind: 'role', name: roles[0] } : { kind: 'members' });
		}
	}

	function focus(element: HTMLInputElement) {
		element.focus();
	}
</script>

<div class="rules">
	<section id="section-roles" aria-labelledby="roles-title">
		<h3
			id="roles-title"
			use:tip={{
				label: 'Roles',
				description:
					'Jobs such as a reviewer or a coordinator. You choose who fills them when you start a goal. Without roles, everyone takes part on equal terms.'
			}}
		>
			<Icon name="users" size={16} /> Roles
		</h3>
		<ul class="chips">
			{#each roles as role (role)}
				<li>
					<button
						type="button"
						class="chip"
						class:open={openRole === role}
						aria-label={`Role "${role}"`}
						use:tip={{
							label: role,
							description: blueprint.roles[role].description || 'Rename, describe or remove.'
						}}
						onclick={() => onopenrole(role)}
					>
						<Icon name="user" size={14} />
						<span>{role}</span>
					</button>
				</li>
			{/each}
			<li>
				{#if adding}
					<form
						class="chip new"
						onsubmit={(event) => {
							event.preventDefault();
							addRoleNamed();
						}}
					>
						<Icon name="user-plus" size={14} />
						<input
							type="text"
							bind:value={newRole}
							aria-label="New role"
							placeholder="reviewer"
							use:focus
							onblur={addRoleNamed}
							onkeydown={(event) => {
								if (event.key === 'Escape') {
									newRole = '';
									adding = false;
								}
							}}
						/>
					</form>
				{:else}
					<button
						type="button"
						class="chip add"
						aria-label="Add a role"
						use:tip={'Add a role'}
						onclick={() => (adding = true)}
					>
						<Icon name="plus" size={14} />
					</button>
				{/if}
			</li>
		</ul>
	</section>

	<section id="section-work" aria-labelledby="work-title">
		<h3 id="work-title" use:tip={'How does work start?'}>
			<Icon name="play" size={16} /> Start
		</h3>
		<Segmented
			label="How does work start?"
			options={startOptions}
			value={start.kind}
			onchange={pickStart}
		/>
		{#if start.kind === 'handed-out'}
			<WhoPicker
				label="Who hands out work?"
				icon="paper-plane-tilt"
				value={start.by}
				{roles}
				allowTaskCreator
				onchange={setStartBy}
			/>
		{/if}
	</section>

	<section id="section-done" aria-labelledby="done-title">
		<h3 id="done-title" use:tip={'When is a task done?'}>
			<Icon name="seal-check" size={16} /> Done
		</h3>
		<DonePicker
			label="When is a task done?"
			value={blueprint.decisions.completion}
			{roles}
			onchange={(completion) => onchange(setDecisions(document, { completion }))}
		/>
	</section>

	<button
		type="button"
		class="more"
		aria-pressed={moreOpen}
		use:tip={{
			label: 'More rules',
			description:
				'Who suggests tasks and shares findings, one final answer, finishing the goal, advice, starting material and task types.'
		}}
		onclick={onopenmore}
	>
		<Icon name="sliders-horizontal" size={16} />
		<span>More</span>
		{#if moreChanged > 0}
			<span class="badge" aria-label={`${moreChanged} changed from the way of working`}
				>{moreChanged}</span
			>
		{/if}
	</button>
</div>

<style>
	.rules {
		display: grid;
		align-content: start;
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
		cursor: default;
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 0.375rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.chip {
		display: inline-flex;
		gap: 0.375rem;
		align-items: center;
		min-height: 2rem;
		max-width: 100%;
		padding: 0 0.75rem 0 0.625rem;
		border: 1px solid var(--color-border);
		border-radius: 999px;
		background: var(--color-surface);
		color: var(--color-text);
		font: var(--text-ui);
	}

	.chip span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.chip :global(.icon) {
		color: var(--color-text-subtle);
	}

	.chip.open {
		border-color: var(--color-accent);
	}

	.chip.add {
		width: 2rem;
		padding: 0;
		justify-content: center;
		background: transparent;
	}

	.chip.new input {
		width: 8rem;
		min-height: 0;
		padding: 0;
		border: 0;
		background: transparent;
	}

	.chip.new input:focus-visible {
		outline: 0;
	}

	.chip.new:focus-within {
		border-color: var(--color-accent);
	}

	.more[aria-pressed='true'] {
		border-color: var(--color-accent);
	}

	.more {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		justify-self: start;
	}

	.badge {
		display: grid;
		place-items: center;
		min-width: 1.25rem;
		height: 1.25rem;
		padding: 0 0.25rem;
		border-radius: 999px;
		background: var(--color-accent);
		color: var(--color-bg);
		font: 500 0.6875rem / 1 var(--font-mono);
	}
</style>
