<!--
	Chooses who a rule names: anyone, one of the roles, or a role made here.
	"New role" asks for a name in place, so roles are made where they are needed.
-->
<script lang="ts">
	import { usableName } from '../model/edit.ts';
	import type { Who } from '../model/line.ts';

	let {
		label,
		value,
		roles,
		anyone = null,
		nobody = null,
		kept = null,
		onpick,
		onnewrole
	}: {
		/** The visible question this field answers. */
		label: string;
		/** The current choice; null when nothing here is chosen yet. */
		value: Who | { kind: 'nobody' } | null;
		roles: string[];
		/** The words for "anyone", or null when a role is required. */
		anyone?: string | null;
		/** The words for "nobody", or null when that is not a choice. */
		nobody?: string | null;
		/** A choice from elsewhere that this page keeps but does not offer. */
		kept?: string | null;
		onpick: (who: Who | { kind: 'nobody' }) => void;
		onnewrole: (name: string) => void;
	} = $props();

	const id = $props.id();
	let naming = $state(false);
	let name = $state('');
	let error = $state('');

	const current = $derived(
		naming
			? 'new'
			: value === null
				? kept === null
					? ''
					: 'kept'
				: value.kind === 'role'
					? `role:${value.name}`
					: value.kind
	);
	// With no roles and no other choice, the only thing to do is name one.
	const mustName = $derived(roles.length === 0 && anyone === null && nobody === null);

	function select(event: Event) {
		const choice = (event.target as HTMLSelectElement).value;
		error = '';
		if (choice === 'new') {
			naming = true;
			return;
		}
		naming = false;
		if (choice === 'anyone') onpick({ kind: 'anyone' });
		else if (choice === 'nobody') onpick({ kind: 'nobody' });
		else if (choice.startsWith('role:')) onpick({ kind: 'role', name: choice.slice(5) });
	}

	function add(event: Event) {
		event.preventDefault();
		const wanted = name.trim();
		if (wanted === '') {
			error = 'Give the role a name.';
			return;
		}
		if (!usableName(wanted)) {
			error = 'That name cannot be used.';
			return;
		}
		if (roles.includes(wanted)) {
			naming = false;
			name = '';
			onpick({ kind: 'role', name: wanted });
			return;
		}
		naming = false;
		name = '';
		error = '';
		onnewrole(wanted);
	}

	function focus(element: HTMLInputElement) {
		element.focus();
	}
</script>

<div class="who">
	<label class="label" for={id}>{label}</label>
	{#if !mustName}
		<select {id} value={current} onchange={select}>
			{#if current === ''}
				<option value="" disabled>Choose…</option>
			{/if}
			{#if anyone !== null}
				<option value="anyone">{anyone}</option>
			{/if}
			{#if nobody !== null}
				<option value="nobody">{nobody}</option>
			{/if}
			{#each roles as role (role)}
				<option value={`role:${role}`}>{role}</option>
			{/each}
			{#if value?.kind === 'role' && !roles.includes(value.name)}
				<option value={`role:${value.name}`}>{value.name} (missing role)</option>
			{/if}
			{#if kept !== null && value === null}
				<option value="kept">{kept} (kept as it is)</option>
			{/if}
			<option value="new">New role…</option>
		</select>
	{/if}
	{#if naming || mustName}
		<form class="new" onsubmit={add}>
			<input
				id={mustName ? id : undefined}
				type="text"
				bind:value={name}
				aria-label="Name of the new role"
				placeholder="Name it"
				use:focus
				oninput={() => (error = '')}
			/>
			<button type="submit">Add role</button>
		</form>
		{#if error}
			<p class="error" role="alert">{error}</p>
		{:else if mustName}
			<p class="hint">Nothing changes until the role has a name.</p>
		{/if}
	{/if}
</div>

<style>
	.who {
		display: grid;
		gap: 0.25rem;
		min-width: 0;
	}

	.new {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.375rem;
	}

	.error {
		color: var(--color-accent);
	}

	.hint {
		color: var(--color-text-subtle);
	}
</style>
