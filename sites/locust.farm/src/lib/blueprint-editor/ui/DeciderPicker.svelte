<!-- Chooses one decider: a role that will have exactly one person, or nobody when that is allowed. -->
<script lang="ts">
	import type { Authority } from '../contract/types.ts';
	import { shortKey } from '../model/words.ts';
	import Icon from './Icon.svelte';
	import type { IconName } from './icons.ts';
	import { tip } from './tooltip.ts';

	let {
		value,
		roles,
		label,
		icon,
		text,
		description,
		noneLabel = null,
		onchange
	}: {
		value: Authority | null;
		roles: string[];
		label: string;
		icon: IconName;
		/** A short visible label beside the icon, for the side panel. */
		text?: string;
		description?: string;
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

<div class="field inline" class:labelled={text}>
	<label
		for={id}
		use:tip={{
			label,
			description:
				roles.length === 0
					? 'To choose someone, first add a role.'
					: (description ?? 'The role must have exactly one person when a goal starts.')
		}}
	>
		<Icon name={icon} size={16} />
		{#if text}
			<span class="short" aria-hidden="true">{text}</span>
		{/if}
		<span class="visually-hidden">{label}</span>
	</label>
	<select {id} value={current} onchange={select}>
		{#if noneLabel !== null}
			<option value="none">{noneLabel}</option>
		{/if}
		{#each roles as role (role)}
			<option value={`role:${role}`}>{role}</option>
		{/each}
		{#if value?.kind === 'role' && !roles.includes(value.name)}
			<option value={`role:${value.name}`}>{value.name} (missing role)</option>
		{/if}
		{#if value?.kind === 'participant'}
			<option value="kept">A specific person (key {shortKey(value.key)})</option>
		{/if}
	</select>
</div>
