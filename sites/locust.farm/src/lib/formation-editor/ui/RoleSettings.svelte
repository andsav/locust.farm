<!-- One role's name and description, shown in the side panel. -->
<script lang="ts">
	import type { EditorDocument } from '../model/document.ts';
	import { describeRole, renameRole, roleUses } from '../model/edit.ts';

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

	const formation = $derived(document.formation);
	const uses = $derived(roleUses(formation, name));

	function rename(event: Event) {
		const input = event.target as HTMLInputElement;
		const to = input.value.trim();
		if (to === name) return;
		if (to === '' || Object.hasOwn(formation.roles, to)) {
			input.value = name;
			onannounce(to === '' ? 'A role needs a name.' : `There is already a role called "${to}".`);
			return;
		}
		onchange(renameRole(document, name, to));
		onrename(to);
		onannounce(`Renamed "${name}" to "${to}" everywhere it is used.`);
	}
</script>

<div class="settings">
	<label class="field">
		<span class="label">Name</span>
		<input type="text" value={name} onchange={rename} />
	</label>
	<label class="field">
		<span class="label">What this role does</span>
		<textarea
			rows="3"
			value={formation.roles[name].description}
			onchange={(event) =>
				onchange(describeRole(document, name, (event.target as HTMLTextAreaElement).value))}
		></textarea>
	</label>
	<p class="uses">
		{uses === 0
			? 'Not used by any rule.'
			: uses === 1
				? 'Used by 1 rule.'
				: `Used by ${uses} rules.`}
		Members are put into this role later, in Locust.
	</p>
</div>

<style>
	.settings {
		display: grid;
		gap: 1rem;
	}

	.uses {
		color: var(--color-text-subtle);
	}
</style>
