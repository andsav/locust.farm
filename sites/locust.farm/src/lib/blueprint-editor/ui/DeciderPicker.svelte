<!-- Chooses one decider: a role that will have exactly one person, or nobody when that is allowed. -->
<script lang="ts">
	import type { Authority } from '../contract/types.ts';
	import { shortKey } from '../model/words.ts';

	let {
		value,
		roles,
		label,
		noneLabel = null,
		onchange
	}: {
		value: Authority | null;
		roles: string[];
		label: string;
		/** The choice for "nobody"; null when a decider is required. */
		noneLabel?: string | null;
		onchange: (value: Authority | null) => void;
	} = $props();

	const id = $props.id();

	const current = $derived(
		value === null ? 'none' : value.kind === 'role' ? `role:${value.name}` : 'kept'
	);

	function select(event: Event) {
		const choice = (event.target as HTMLSelectElement).value;
		if (choice === 'kept') return;
		onchange(choice === 'none' ? null : { kind: 'role', name: choice.slice(5) });
	}
</script>

<label class="field" for={id}>
	<span class="label">{label}</span>
	<select {id} value={current} onchange={select}>
		{#if noneLabel !== null}
			<option value="none">{noneLabel}</option>
		{/if}
		{#each roles as role (role)}
			<option value={`role:${role}`}>The person in the "{role}" role</option>
		{/each}
		{#if value?.kind === 'role' && !roles.includes(value.name)}
			<option value={`role:${value.name}`}>The "{value.name}" role (not in the list)</option>
		{/if}
		{#if value?.kind === 'participant'}
			<option value="kept">A specific person (key {shortKey(value.key)}), kept as it is</option>
		{/if}
	</select>
	{#if roles.length === 0 && value === null && noneLabel !== null}
		<span class="help">To choose someone, first add a role under "Who is involved?".</span>
	{:else}
		<span class="help">This role must have exactly one person when a goal starts.</span>
	{/if}
</label>
