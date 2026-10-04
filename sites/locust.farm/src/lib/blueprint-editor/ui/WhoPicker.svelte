<!-- Chooses a group of people: everyone in the goal, a role, or one of the special choices allowed here. -->
<script lang="ts">
	import type { Selector } from '../contract/types.ts';
	import { who } from '../model/words.ts';
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
		allowTaskCreator = false,
		allowAuthor = false,
		allowNobody = false,
		onchange
	}: {
		value: Selector;
		roles: string[];
		/** The question. Shown in the tooltip and read by screen readers. */
		label: string;
		icon: IconName;
		/** A short visible label beside the icon, for the side panel. */
		text?: string;
		description?: string;
		allowTaskCreator?: boolean;
		allowAuthor?: boolean;
		allowNobody?: boolean;
		onchange: (value: Selector) => void;
	} = $props();

	const id = $props.id();

	const current = $derived(
		value.kind === 'role'
			? `role:${value.name}`
			: value.kind === 'any' || value.kind === 'participant'
				? 'kept'
				: value.kind
	);

	function select(event: Event) {
		const choice = (event.target as HTMLSelectElement).value;
		if (choice === 'kept') return;
		if (choice.startsWith('role:')) onchange({ kind: 'role', name: choice.slice(5) });
		else onchange({ kind: choice } as Selector);
	}
</script>

<div class="field inline" class:labelled={text}>
	<label for={id} use:tip={{ label, description }}>
		<Icon name={icon} size={16} />
		{#if text}
			<span class="short" aria-hidden="true">{text}</span>
		{/if}
		<span class="visually-hidden">{label}</span>
	</label>
	<select {id} value={current} onchange={select}>
		<option value="members">Everyone</option>
		{#each roles as role (role)}
			<option value={`role:${role}`}>{role}</option>
		{/each}
		{#if value.kind === 'role' && !roles.includes(value.name)}
			<option value={`role:${value.name}`}>{value.name} (missing role)</option>
		{/if}
		{#if allowTaskCreator || value.kind === 'task_creator'}
			<option value="task_creator">Whoever created the task</option>
		{/if}
		{#if allowAuthor || value.kind === 'contribution_author'}
			<option value="contribution_author">The author of the work</option>
		{/if}
		{#if allowNobody || value.kind === 'nobody'}
			<option value="nobody">Nobody</option>
		{/if}
		{#if current === 'kept'}
			<option value="kept">{who(value)} (kept as it is)</option>
		{/if}
	</select>
</div>
