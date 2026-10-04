<!-- The blueprint editor: ways of working, rules, the stage map, the summary and the copy button. -->
<script lang="ts">
	import './editor.css';
	import { onMount } from 'svelte';
	import { copyText } from '../../onboarding/clipboard.ts';
	import StageMap from '../canvas/StageMap.svelte';
	import { inspectBlueprint } from '../contract/inspect.ts';
	import { changedFromWay } from '../model/answers.ts';
	import { pageChecks } from '../model/checks.ts';
	import { blueprintText, type EditorDocument } from '../model/document.ts';
	import { applyWay, newDocument, removeStage, setName, stageOrder } from '../model/edit.ts';
	import { record, redo, startHistory, undo, type History } from '../model/history.ts';
	import { wayOfWorking } from '../model/presets.ts';
	import { summarize } from '../model/words.ts';
	import { openText } from '../prompt/open.ts';
	import { blocks, draftId } from '../prompt/prompt.ts';
	import { decodeLink, encodeLink, LONG_LINK } from '../storage/link.ts';
	import {
		browserStore,
		listSaved,
		loadSaved,
		newRecordId,
		removeSaved,
		save,
		type KeyValueStore,
		type SavedSummary
	} from '../storage/saved.ts';
	import CopyPanel from './CopyPanel.svelte';
	import { fromDiagnostics, fromPageChecks, perStage, type Place } from './problems.ts';
	import RulesPanel from './RulesPanel.svelte';
	import SidePanel from './SidePanel.svelte';
	import StageList from './StageList.svelte';
	import StageSettings from './StageSettings.svelte';
	import Summary from './Summary.svelte';
	import WayPicker from './WayPicker.svelte';

	let history = $state.raw<History>(startHistory(newDocument()));
	const document = $derived(history.present);

	let pickerOpen = $state(true);
	let selected = $state<string | null>(null);
	let panelOpen = $state(false);
	let announcement = $state('');
	let saveStatus = $state('');
	let banner = $state<string | null>(null);
	let compact = $state(false);

	let store: KeyValueStore | null = null;
	let recordId = newRecordId();
	let origin: 'new' | 'link' | 'import' = 'new';

	const inspection = $derived(inspectBlueprint(document.blueprint));
	const checks = $derived(pageChecks(document.blueprint));
	const problems = $derived([
		...fromDiagnostics(inspection.diagnostics),
		...fromPageChecks(checks)
	]);
	const stageProblems = $derived(perStage(problems));
	const blocked = $derived(
		checks.some((check) => check.blocksCopy)
			? 'Remove the invisible characters listed under "What this means" before copying.'
			: null
	);
	const lines = $derived(summarize(document));
	const changed = $derived(
		document.way ? changedFromWay(document, applyWay(newDocument(), document.way)) : []
	);
	const order = $derived(stageOrder(document));

	function announce(message: string) {
		announcement = '';
		queueMicrotask(() => (announcement = message));
	}

	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	function scheduleSave() {
		clearTimeout(saveTimer);
		saveTimer = setTimeout(async () => {
			if (!store) {
				saveStatus =
					"Not saved: this browser isn't keeping data for this page. Use Copy link or Download to keep your work.";
				return;
			}
			const data = await blocks(document);
			const ok = save(store, {
				id: recordId,
				name: document.name,
				way: document.way,
				origin,
				saved: new Date().toISOString(),
				data: data.text
			});
			saveStatus = ok
				? 'Saved in this browser.'
				: 'Not saved: this browser refused to store it. Use Copy link or Download to keep your work.';
		}, 400);
	}

	function change(next: EditorDocument) {
		const before = history;
		history = record(history, next);
		if (history !== before) scheduleSave();
	}

	function pickWay(id: string) {
		change(applyWay(document, id));
		pickerOpen = false;
		selected = null;
		panelOpen = false;
		announce(
			`Loaded the ${wayOfWorking(id)?.title ?? id} way of working. You can change anything below.`
		);
	}

	function select(name: string | null) {
		selected = name;
		panelOpen = name !== null;
	}

	function show(place: Place) {
		if (place.kind === 'stage') {
			if (Object.hasOwn(document.blueprint.flow, place.name)) select(place.name);
			return;
		}
		const section = window.document.getElementById(`section-${place.id}`);
		const details = section?.closest('details');
		if (details) details.open = true;
		section?.scrollIntoView({ block: 'center' });
		section?.querySelector<HTMLElement>('input, select, textarea, button')?.focus();
	}

	function doUndo() {
		history = undo(history);
		if (selected && !Object.hasOwn(history.present.blueprint.flow, selected)) select(null);
		scheduleSave();
		announce('Undone.');
	}

	function doRedo() {
		history = redo(history);
		if (selected && !Object.hasOwn(history.present.blueprint.flow, selected)) select(null);
		scheduleSave();
		announce('Redone.');
	}

	function onkeydown(event: KeyboardEvent) {
		const target = event.target as HTMLElement;
		if (target.closest('input, textarea, select, [contenteditable]')) return;
		const mod = event.metaKey || event.ctrlKey;
		if (mod && event.key.toLowerCase() === 'z') {
			event.preventDefault();
			if (event.shiftKey) doRedo();
			else doUndo();
		} else if (mod && event.key.toLowerCase() === 'y') {
			event.preventDefault();
			doRedo();
		} else if (event.key === 'Escape' && panelOpen) {
			select(null);
		}
	}

	function load(next: EditorDocument, from: 'new' | 'link' | 'import', id = newRecordId()) {
		history = startHistory(next);
		recordId = id;
		origin = from;
		select(null);
		pickerOpen = false;
	}

	// Saved in this browser.
	let savedDialog: HTMLDialogElement | undefined = $state();
	let saved = $state<SavedSummary[]>([]);

	function openSaved() {
		saved = store ? listSaved(store) : [];
		savedDialog?.showModal();
	}

	function openRecord(id: string) {
		if (!store) return;
		const found = loadSaved(store, id);
		const opened = found ? openText(found.data) : null;
		if (found && opened?.ok) {
			load(opened.document, found.origin, found.id);
			announce(`Opened "${found.name || 'Untitled blueprint'}".`);
		}
		savedDialog?.close();
	}

	function deleteRecord(id: string) {
		if (!store) return;
		removeSaved(store, id);
		saved = listSaved(store);
		if (id === recordId) recordId = newRecordId();
	}

	// Open a blueprint.
	let openDialog: HTMLDialogElement | undefined = $state();
	let pasted = $state('');
	let openMessage = $state('');

	function openFrom(text: string) {
		const opened = openText(text);
		if (!opened.ok) {
			openMessage = opened.message;
			return;
		}
		load(opened.document, 'import');
		scheduleSave();
		openDialog?.close();
		pasted = '';
		openMessage = '';
		announce(
			opened.note ?? 'Opened the blueprint. Your earlier work is still under Saved in this browser.'
		);
	}

	async function openFile(event: Event) {
		const file = (event.target as HTMLInputElement).files?.[0];
		if (file) openFrom(await file.text());
	}

	// Links and downloads.
	async function copyLink() {
		const data = await blocks(document);
		const fragment = await encodeLink(data.text);
		const url = `${location.origin}${location.pathname}#${fragment}`;
		const result = await copyText(url, navigator.clipboard);
		const privacy =
			'Anyone with the link can read this blueprint, and it stays in browser history.';
		announce(
			result === 'copied'
				? `Link copied. ${privacy}${url.length > LONG_LINK ? ' This link is long; some chat apps may cut it, so Download may work better.' : ''}`
				: 'Copying the link did not work. Use Download instead.'
		);
	}

	function download() {
		const blob = new Blob([blueprintText(document.blueprint)], { type: 'application/json' });
		const link = window.document.createElement('a');
		link.href = URL.createObjectURL(blob);
		link.download = `${draftId(document.name)}.json`;
		link.click();
		setTimeout(() => URL.revokeObjectURL(link.href), 1000);
	}

	function startOver() {
		load(newDocument(), 'new');
		pickerOpen = true;
		scheduleSave();
		announce('Started a new blueprint. The earlier one is under Saved in this browser.');
	}

	onMount(() => {
		store = browserStore();
		const media = matchMedia('(max-width: 48rem)');
		compact = media.matches;
		const onMedia = () => (compact = media.matches);
		media.addEventListener('change', onMedia);

		(async () => {
			const link = location.hash ? await decodeLink(location.hash) : null;
			if (link) {
				window.history.replaceState(null, '', location.pathname + location.search);
				if (link.ok) {
					const opened = openText(link.text);
					if (opened.ok) {
						load(opened.document, 'link');
						banner =
							'Opened from a link. If someone else made it, read its names and advice before you copy it.';
						scheduleSave();
						return;
					}
					banner = opened.message;
				} else {
					banner = link.message;
				}
			}
			const latest = store ? listSaved(store)[0] : undefined;
			const found = latest && store ? loadSaved(store, latest.id) : null;
			const opened = found ? openText(found.data) : null;
			if (found && opened?.ok) {
				load(opened.document, found.origin, found.id);
				return;
			}
			scheduleSave();
		})();

		return () => media.removeEventListener('change', onMedia);
	});
</script>

<svelte:window {onkeydown} />

<div class="blueprint-editor">
	{#if banner}
		<div class="banner" role="status">
			<p>{banner}</p>
			<button type="button" onclick={() => (banner = null)}>Dismiss</button>
		</div>
	{/if}

	{#if pickerOpen}
		<section class="picker" aria-label="Ways of working">
			<WayPicker current={document.way} onpick={pickWay} />
			<p class="muted">
				Pick the closest one. You can change any of it below. Nothing is sent anywhere until you
				copy the prompt into your agent.
			</p>
			{#if document.way}
				<button type="button" onclick={() => (pickerOpen = false)}>
					Continue with {wayOfWorking(document.way)?.title ?? 'this way'}
				</button>
			{/if}
		</section>
	{/if}

	<div class="bar">
		<label class="field name">
			<span class="label">Name of this blueprint</span>
			<input
				type="text"
				value={document.name}
				placeholder="for example: Sync research"
				onchange={(event) => change(setName(document, (event.target as HTMLInputElement).value))}
			/>
		</label>
		<div class="actions">
			<button type="button" onclick={doUndo} disabled={history.past.length === 0}>Undo</button>
			<button type="button" onclick={doRedo} disabled={history.future.length === 0}>Redo</button>
			<button type="button" onclick={() => openDialog?.showModal()}>Open a blueprint</button>
			<button type="button" onclick={openSaved}>Saved in this browser</button>
			<button type="button" onclick={copyLink}>Copy link</button>
			<button type="button" onclick={download}>Download</button>
			<button type="button" onclick={startOver}>Start over</button>
		</div>
		<p class="save-status">{saveStatus}</p>
	</div>

	<div class="columns">
		<div class="left">
			<RulesPanel
				{document}
				{changed}
				onchange={change}
				onchangeway={() => {
					pickerOpen = true;
					window.scrollTo({ top: 0 });
				}}
				onannounce={announce}
			/>
		</div>

		<div class="middle">
			<section aria-labelledby="stages-title" class="stages">
				<h2 id="stages-title">Stages</h2>
				<p class="help">
					Optional. Add stages when work should move from one step to the next, such as draft, then
					review. Arrows show what each stage waits for.
				</p>
				<div class="map-area" class:compact>
					<StageMap
						{document}
						{selected}
						{panelOpen}
						panelWidth={0}
						problems={stageProblems}
						{compact}
						onchange={change}
						onselect={select}
						onannounce={announce}
					/>
				</div>
				<StageList {document} problems={stageProblems} onopen={select} />
			</section>
		</div>

		<div class="right">
			<Summary
				{lines}
				{problems}
				explanation={inspection.explanation}
				hasStages={order.length > 0}
				onshow={show}
			/>
			<CopyPanel {document} hasErrors={inspection.diagnostics.length > 0} {blocked} />
			{#if selected && Object.hasOwn(document.blueprint.flow, selected)}
				<div class="panel-host" class:compact>
					<SidePanel
						open={panelOpen}
						number={order.indexOf(selected) + 1}
						title={selected}
						onclose={() => select(null)}
					>
						<StageSettings
							{document}
							name={selected}
							onchange={change}
							onrename={(to) => (selected = to)}
							onannounce={announce}
						/>
						{#snippet footer()}
							<button
								type="button"
								onclick={() => {
									const name = selected!;
									change(removeStage(document, name));
									select(null);
									announce(`Removed stage "${name}". Undo brings it back.`);
								}}
							>
								Remove this stage
							</button>
							<span class="help">{saveStatus}</span>
						{/snippet}
					</SidePanel>
				</div>
			{/if}
		</div>
	</div>

	<p class="visually-hidden" aria-live="polite">{announcement}</p>

	<dialog bind:this={openDialog} aria-labelledby="open-title">
		<h2 id="open-title">Open a blueprint</h2>
		<p class="help">
			Paste blueprint JSON, a prompt copied from this page, or your agent's whole reply. Or choose a
			file.
		</p>
		<textarea rows="8" bind:value={pasted} aria-label="Pasted blueprint"></textarea>
		<label class="field">
			<span class="label">Or choose a file</span>
			<input type="file" accept=".json,.txt,application/json,text/plain" onchange={openFile} />
		</label>
		{#if openMessage}
			<p class="message" role="alert">{openMessage}</p>
		{/if}
		<div class="dialog-actions">
			<button type="button" onclick={() => openFrom(pasted)} disabled={pasted.trim() === ''}
				>Open</button
			>
			<button type="button" onclick={() => openDialog?.close()}>Cancel</button>
		</div>
	</dialog>

	<dialog bind:this={savedDialog} aria-labelledby="saved-title">
		<h2 id="saved-title">Saved in this browser</h2>
		<p class="help">
			Browsers can clear this, and Safari does after seven days without a visit. Use Download or
			Copy link to keep a copy.
		</p>
		{#if saved.length === 0}
			<p class="muted">Nothing saved yet.</p>
		{:else}
			<ul class="items">
				{#each saved as item (item.id)}
					<li class="item row">
						<span class="item-name">
							{item.name || 'Untitled blueprint'}
							<span class="help">
								{item.origin === 'link'
									? 'From a link · '
									: item.origin === 'import'
										? 'Opened · '
										: ''}{new Date(item.saved).toLocaleString()}
							</span>
						</span>
						<button type="button" onclick={() => openRecord(item.id)}>Open</button>
						<button type="button" class="quiet" onclick={() => deleteRecord(item.id)}>Delete</button
						>
					</li>
				{/each}
			</ul>
		{/if}
		<div class="dialog-actions">
			<button type="button" onclick={() => savedDialog?.close()}>Close</button>
		</div>
	</dialog>
</div>

<style>
	.blueprint-editor {
		display: grid;
		gap: 1.5rem;
	}

	.banner {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		align-items: center;
		padding: 0.75rem 1rem;
		border: 1px solid var(--color-accent);
	}

	.banner p {
		flex: 1;
	}

	.picker {
		display: grid;
		gap: 0.75rem;
	}

	.picker button {
		justify-self: start;
	}

	.bar {
		display: grid;
		grid-template-columns: minmax(12rem, 22rem) 1fr;
		gap: 0.75rem 1.5rem;
		align-items: end;
		padding-bottom: 1rem;
		border-bottom: var(--border-hairline);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.375rem;
	}

	.save-status {
		grid-column: 1 / -1;
		color: var(--color-text-faint);
		font: var(--text-label);
	}

	.columns {
		display: grid;
		grid-template-columns: minmax(18rem, 22rem) minmax(0, 1fr) minmax(18rem, 22rem);
		gap: 1.5rem;
		align-items: start;
	}

	.left,
	.right {
		display: grid;
		gap: 1.5rem;
	}

	.right {
		position: sticky;
		top: 1rem;
		min-height: min(80vh, 44rem);
	}

	.stages {
		display: grid;
		gap: 0.75rem;
	}

	.map-area {
		position: relative;
		height: min(70vh, 40rem);
		overflow: hidden;
	}

	.map-area.compact {
		height: 18rem;
	}

	.panel-host {
		position: absolute;
		inset: 0 -1px 0 0;
		overflow: hidden;
		pointer-events: none;
	}

	.panel-host :global(.panel) {
		width: 100%;
	}

	.panel-host > :global(*) {
		pointer-events: auto;
	}

	.panel-host.compact {
		position: fixed;
		z-index: 40;
	}

	dialog {
		width: min(36rem, calc(100% - 2rem));
		padding: 1.25rem;
		border: 1px solid var(--color-border);
		background: var(--color-bg);
		color: var(--color-text);
	}

	dialog::backdrop {
		background: rgb(0 0 0 / 0.7);
	}

	dialog[open] {
		display: grid;
		gap: 0.75rem;
	}

	.dialog-actions {
		display: flex;
		gap: 0.5rem;
	}

	.message {
		color: var(--color-text);
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--color-accent);
	}

	@media (max-width: 64rem) {
		.columns {
			grid-template-columns: minmax(16rem, 20rem) minmax(0, 1fr);
		}

		.right {
			position: static;
			grid-column: 1 / -1;
			grid-template-columns: 1fr 1fr;
		}
	}

	@media (max-width: 48rem) {
		.bar,
		.columns,
		.right {
			grid-template-columns: 1fr;
		}
	}
</style>
