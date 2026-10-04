<!-- Chooses a group of people: everyone in the goal, a role, or one of the special choices allowed here. -->
<script lang="ts">
	import type { Selector } from '../contract/types.ts';
	import { who } from '../model/words.ts';

	let {
		value,
		roles,
		label,
		allowTaskCreator = false,
		allowAuthor = false,
		allowNobody = false,
		onchange
	}: {
		value: Selector;
		roles: string[];
		label: string;
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

<label class="field" for={id}>
	<span class="label">{label}</span>
	<select {id} value={current} onchange={select}>
		<option value="members">Everyone in the goal</option>
		{#each roles as role (role)}
			<option value={`role:${role}`}>People in the "{role}" role</option>
		{/each}
		{#if value.kind === 'role' && !roles.includes(value.name)}
			<option value={`role:${value.name}`}>The "{value.name}" role (not in the list)</option>
		{/if}
		{#if allowTaskCreator || value.kind === 'task_creator'}
			<option value="task_creator">The person who created the task</option>
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
</label>
