<!--
	The blueprint editor: the ways of working, one toolbar, the rules beside a
	large stage map, and a side panel for everything else.
-->
<script lang="ts">
	import './editor.css';
	import { onMount, tick } from 'svelte';
	import StageMap from '../canvas/StageMap.svelte';
	import { inspectBlueprint } from '../contract/inspect.ts';
	import { changedFromWay } from '../model/answers.ts';
	import { pageChecks } from '../model/checks.ts';
	import { blueprintText, type EditorDocument } from '../model/document.ts';
	import {
		applyWay,
		newDocument,
		removeRole,
		removeStage,
		roleUses,
		setName,
		stageOrder
	} from '../model/edit.ts';
	import { record, redo, startHistory, undo, type History } from '../model/history.ts';
	import { wayOfWorking } from '../model/presets.ts';
	import { summarize } from '../model/words.ts';
	import { openText } from '../prompt/open.ts';
	import {
		blocks,
		buildPrompt,
		draftId,
		LARGE_PROMPT,
		type BuiltPrompt,
		type Intent
	} from '../prompt/prompt.ts';
	import { copyText } from '../../onboarding/clipboard.ts';
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
	import CopyBar from './CopyBar.svelte';
	import Icon from './Icon.svelte';
	import type { IconName } from './icons.ts';
	import MoreSettings from './MoreSettings.svelte';
	import OpenBlueprint from './OpenBlueprint.svelte';
	import Problems from './Problems.svelte';
	import { fromDiagnostics, fromPageChecks, perStage, type Place } from './problems.ts';
	import RoleSettings from './RoleSettings.svelte';
	import RulesPanel from './RulesPanel.svelte';
	import SavedList from './SavedList.svelte';
	import SidePanel from './SidePanel.svelte';
	import StageList from './StageList.svelte';
	import StageSettings from './StageSettings.svelte';
	import Summary from './Summary.svelte';
	import { tip } from './tooltip.ts';
	import WayPicker from './WayPicker.svelte';

	type Panel =
		| { kind: 'stage'; name: string }
		| { kind: 'role'; name: string }
		| { kind: 'more' | 'problems' | 'words' | 'prompt' | 'open' | 'saved' };

	let history = $state.raw<History>(startHistory(newDocument()));
	const document = $derived(history.present);

	let panel = $state<Panel | null>(null);
	/** Counts loaded blueprints, so the map frames each one afresh. */
	let frame = $state(0);
	let announcement = $state('');
	let saveStatus = $state<{ ok: boolean; text: string }>({ ok: true, text: '' });
	let banner = $state<string | null>(null);
	let compact = $state(false);

	let store: KeyValueStore | null = null;
	let recordId = $state(newRecordId());
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
			? 'Remove the invisible characters listed under problems before copying.'
			: null
	);
	const lines = $derived(summarize(document));
	const changed = $derived(
		document.way ? changedFromWay(document, applyWay(newDocument(), document.way)) : []
	);
	const moreChanged = $derived(
		changed.filter((area) =>
			['final answer', 'finish', 'advice', 'starting material', 'task types'].includes(area)
		).length
	);
	const order = $derived(stageOrder(document));
	const selected = $derived(panel?.kind === 'stage' ? panel.name : null);

	// The prompt follows every change.
	let intent = $state<Intent>('add');
	let prompt = $state<BuiltPrompt | null>(null);
	$effect(() => {
		const current = { document, intent, hasErrors: inspection.diagnostics.length > 0 };
		let cancelled = false;
		buildPrompt(current.document, current.intent, current.hasErrors).then((built) => {
			if (!cancelled) prompt = built;
		});
		return () => {
			cancelled = true;
		};
	});
	let promptElement: HTMLPreElement | undefined = $state();

	function announce(message: string) {
		announcement = '';
		queueMicrotask(() => (announcement = message));
	}

	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	function scheduleSave() {
		clearTimeout(saveTimer);
		saveTimer = setTimeout(async () => {
			if (!store) {
				saveStatus = {
					ok: false,
					text: "Not saved: this browser isn't keeping data for this page. Download or copy a link to keep your work."
				};
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
				? { ok: true, text: 'Saved in this browser.' }
				: {
						ok: false,
						text: 'Not saved: this browser refused to store it. Download or copy a link to keep your work.'
					};
		}, 400);
	}

	function change(next: EditorDocument) {
		const before = history;
		history = record(history, next);
		if (history !== before) scheduleSave();
	}

	/** Closes a stage or role panel whose stage or role no longer exists. */
	function keepPanelValid() {
		const blueprint = history.present.blueprint;
		if (panel?.kind === 'stage' && !Object.hasOwn(blueprint.flow, panel.name)) panel = null;
		if (panel?.kind === 'role' && !Object.hasOwn(blueprint.roles, panel.name)) panel = null;
	}

	function pickWay(id: string) {
		change(applyWay(document, id));
		frame += 1;
		panel = null;
		announce(`Loaded the ${wayOfWorking(id)?.title ?? id} way of working.`);
	}

	function open(next: Panel | null) {
		panel = next;
	}

	function toggle(kind: 'more' | 'problems' | 'words' | 'prompt' | 'open' | 'saved') {
		if (panel?.kind === kind) {
			panel = null;
			return;
		}
		if (kind === 'saved') saved = store ? listSaved(store) : [];
		if (kind === 'open') openMessage = '';
		panel = { kind };
	}

	async function show(place: Place) {
		if (place.kind === 'stage') {
			if (Object.hasOwn(document.blueprint.flow, place.name))
				panel = { kind: 'stage', name: place.name };
			return;
		}
		const inRail = place.id === 'roles' || place.id === 'work' || place.id === 'done';
		panel = inRail ? null : { kind: 'more' };
		await tick();
		const section = window.document.getElementById(`section-${place.id}`);
		section?.scrollIntoView({ block: 'center' });
		section?.querySelector<HTMLElement>('input, select, textarea, button')?.focus();
	}

	function doUndo() {
		history = undo(history);
		keepPanelValid();
		scheduleSave();
		announce('Undone.');
	}

	function doRedo() {
		history = redo(history);
		keepPanelValid();
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
		} else if (event.key === 'Escape' && panel) {
			panel = null;
		}
	}

	function load(next: EditorDocument, from: 'new' | 'link' | 'import', id = newRecordId()) {
		history = startHistory(next);
		recordId = id;
		origin = from;
		panel = null;
		frame += 1;
	}

	// Saved in this browser.
	let saved = $state<SavedSummary[]>([]);

	function openRecord(id: string) {
		if (!store) return;
		const found = loadSaved(store, id);
		const opened = found ? openText(found.data) : null;
		if (found && opened?.ok) {
			load(opened.document, found.origin, found.id);
			announce(`Opened "${found.name || 'Untitled blueprint'}".`);
		}
	}

	function deleteRecord(id: string) {
		if (!store) return;
		removeSaved(store, id);
		saved = listSaved(store);
		if (id === recordId) recordId = newRecordId();
	}

	// Open a blueprint.
	let openMessage = $state('');

	function openFrom(text: string) {
		const opened = openText(text);
		if (!opened.ok) {
			openMessage = opened.message;
			return;
		}
		load(opened.document, 'import');
		scheduleSave();
		openMessage = '';
		announce(
			opened.note ?? 'Opened the blueprint. Your earlier work is still under Saved in this browser.'
		);
	}

	// Links and downloads.
	async function copyLink() {
		const data = await blocks(document);
		const fragment = await encodeLink(data.text);
		const url = `${location.origin}${location.pathname}#${fragment}`;
		const result = await copyText(url, navigator.clipboard);
		const privacy =
			'Anyone with the link can read this blueprint, and it stays in browser history.';
		toast(
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
		scheduleSave();
		toast('Started a new blueprint. The earlier one is under Saved in this browser.');
	}

	// A short message under the toolbar, also read by screen readers.
	let toastText = $state('');
	let toastTimer: ReturnType<typeof setTimeout> | undefined;
	function toast(message: string) {
		clearTimeout(toastTimer);
		toastText = message;
		announce(message);
		toastTimer = setTimeout(() => (toastText = ''), 6000);
	}

	async function copyFailed() {
		panel = { kind: 'prompt' };
		await tick();
		if (!promptElement) return;
		const range = window.document.createRange();
		range.selectNodeContents(promptElement);
		const selection = window.getSelection();
		selection?.removeAllRanges();
		selection?.addRange(range);
	}

	const panelTitle = $derived.by((): { title: string; mark: number | IconName } | null => {
		if (!panel) return null;
		switch (panel.kind) {
			case 'stage':
				return { title: panel.name, mark: order.indexOf(panel.name) + 1 };
			case 'role':
				return { title: panel.name, mark: 'user' };
			case 'more':
				return { title: 'More rules', mark: 'sliders-horizontal' };
			case 'problems':
				return {
					title:
						problems.length === 0
							? 'No problems'
							: problems.length === 1
								? '1 thing to check'
								: `${problems.length} things to check`,
					mark: problems.length === 0 ? 'check-circle' : 'warning'
				};
			case 'words':
				return { title: 'In words', mark: 'article' };
			case 'prompt':
				return {
					title: prompt
						? `The prompt, ${(prompt.text.length / 1000).toFixed(1)} thousand characters`
						: 'The prompt',
					mark: 'eye'
				};
			case 'open':
				return { title: 'Open a blueprint', mark: 'folder-open' };
			case 'saved':
				return { title: 'Saved in this browser', mark: 'clock-counter-clockwise' };
		}
	});

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

{#snippet tool(
	label: string,
	icon: IconName,
	action: () => void,
	options: { disabled?: boolean; pressed?: boolean; description?: string } = {}
)}
	<button
		type="button"
		class="tool"
		aria-label={label}
		aria-pressed={options.pressed}
		disabled={options.disabled}
		use:tip={{ label, description: options.description }}
		onclick={action}
	>
		<Icon name={icon} />
	</button>
{/snippet}

<div class="blueprint-editor">
	{#if banner}
		<div class="banner" role="status">
			<Icon name="warning" />
			<p>{banner}</p>
			<button
				type="button"
				class="tool"
				aria-label="Dismiss"
				use:tip={'Dismiss'}
				onclick={() => (banner = null)}
			>
				<Icon name="x" />
			</button>
		</div>
	{/if}

	<WayPicker current={document.way} changed={changed.length} onpick={pickWay} />

	<div class="frame" class:compact>
		<div class="bar">
			<div class="name">
				<input
					type="text"
					value={document.name}
					aria-label="Name of this blueprint"
					placeholder="Untitled blueprint"
					onchange={(event) => change(setName(document, (event.target as HTMLInputElement).value))}
				/>
				<span
					class="saved-mark"
					class:unsaved={!saveStatus.ok}
					role="img"
					aria-label={saveStatus.text || 'Saving'}
					use:tip={saveStatus.text || 'Saving'}
				>
					<Icon name={saveStatus.ok ? 'check' : 'warning'} size={14} />
				</span>
			</div>

			<div class="tools" role="toolbar" aria-label="Blueprint">
				{@render tool('Undo', 'arrow-counter-clockwise', doUndo, {
					disabled: history.past.length === 0,
					description: 'Cmd or Ctrl + Z'
				})}
				{@render tool('Redo', 'arrow-clockwise', doRedo, {
					disabled: history.future.length === 0,
					description: 'Shift + Cmd or Ctrl + Z'
				})}
				<span class="divider" aria-hidden="true"></span>
				{@render tool('Open a blueprint', 'folder-open', () => toggle('open'), {
					pressed: panel?.kind === 'open',
					description: 'From pasted JSON, a prompt from this page, an agent reply or a file.'
				})}
				{@render tool('Saved in this browser', 'clock-counter-clockwise', () => toggle('saved'), {
					pressed: panel?.kind === 'saved'
				})}
				{@render tool('Copy link', 'link', copyLink, {
					description: 'A link that opens this blueprint. The blueprint stays in the link.'
				})}
				{@render tool('Download', 'download-simple', download, {
					description: 'The blueprint as a JSON file.'
				})}
				{@render tool('Start over', 'file-plus', startOver, {
					description: 'The current blueprint stays under Saved in this browser.'
				})}
			</div>

			<span class="spacer"></span>

			<div class="tools" role="toolbar" aria-label="Check and read">
				<button
					type="button"
					class="tool status"
					class:attention={problems.length > 0}
					aria-label={problems.length === 0
						? 'No problems found'
						: problems.length === 1
							? '1 problem'
							: `${problems.length} problems`}
					aria-pressed={panel?.kind === 'problems'}
					use:tip={problems.length === 0
						? 'No problems found'
						: { label: 'Problems', description: problems[0].text }}
					onclick={() => toggle('problems')}
				>
					<Icon name={problems.length === 0 ? 'check-circle' : 'warning'} />
					{#if problems.length > 0}<span class="count">{problems.length}</span>{/if}
				</button>
				{@render tool('In words', 'article', () => toggle('words'), {
					pressed: panel?.kind === 'words',
					description: 'The blueprint in plain sentences, and what Locust will say.'
				})}
				{@render tool('See the prompt', 'eye', () => toggle('prompt'), {
					pressed: panel?.kind === 'prompt'
				})}
				<a
					class="tool"
					href="/docs/next/blueprint-authoring"
					aria-label="Read about blueprints in the manual"
					use:tip={'Read about blueprints in the manual'}
				>
					<Icon name="book-open" />
				</a>
				<a
					class="tool"
					href="/start"
					aria-label="Set up Locust"
					use:tip={{
						label: 'Set up Locust',
						description: 'The prompt needs Locust on your computer. Not set up yet? Start here.'
					}}
				>
					<Icon name="question" />
				</a>
			</div>

			<CopyBar
				{prompt}
				bind:intent
				hasErrors={inspection.diagnostics.length > 0}
				{blocked}
				onfail={copyFailed}
			/>

			{#if toastText}
				<p class="toast">{toastText}</p>
			{/if}
		</div>

		<div class="body">
			<aside class="rail" aria-label="Rules">
				<RulesPanel
					{document}
					{moreChanged}
					openRole={panel?.kind === 'role' ? panel.name : null}
					moreOpen={panel?.kind === 'more'}
					onchange={change}
					onopenrole={(name) => open({ kind: 'role', name })}
					onopenmore={() => toggle('more')}
					onannounce={announce}
				/>
			</aside>

			<div class="canvas">
				<StageMap
					{document}
					{selected}
					panelOpen={selected !== null}
					panelWidth={panel && !compact ? 400 : 0}
					problems={stageProblems}
					{compact}
					{frame}
					onchange={change}
					onselect={(name) => open(name === null ? null : { kind: 'stage', name })}
					onannounce={announce}
				/>

				{#if panel && panelTitle}
					<div class="panel-host" class:compact>
						<SidePanel
							open
							mark={panelTitle.mark}
							title={panelTitle.title}
							onclose={() => (panel = null)}
						>
							{#if panel.kind === 'stage' && Object.hasOwn(document.blueprint.flow, panel.name)}
								<StageSettings
									{document}
									name={panel.name}
									onchange={change}
									onrename={(to) => (panel = { kind: 'stage', name: to })}
									onannounce={announce}
								/>
							{:else if panel.kind === 'role' && Object.hasOwn(document.blueprint.roles, panel.name)}
								<RoleSettings
									{document}
									name={panel.name}
									onchange={change}
									onrename={(to) => (panel = { kind: 'role', name: to })}
									onannounce={announce}
								/>
							{:else if panel.kind === 'more'}
								<MoreSettings {document} onchange={change} />
							{:else if panel.kind === 'problems'}
								<Problems {problems} onshow={show} />
							{:else if panel.kind === 'words'}
								<Summary
									{lines}
									explanation={inspection.explanation}
									hasStages={order.length > 0}
								/>
							{:else if panel.kind === 'prompt'}
								{#if prompt && prompt.text.length > LARGE_PROMPT}
									<p class="faint">
										This prompt is long. Some agents shorten very long pastes; if yours does,
										download the blueprint and give your agent the file.
									</p>
								{/if}
								<pre class="prompt" bind:this={promptElement}>{prompt?.text ?? ''}</pre>
							{:else if panel.kind === 'open'}
								<OpenBlueprint message={openMessage} onopen={openFrom} />
							{:else if panel.kind === 'saved'}
								<SavedList {saved} current={recordId} onopen={openRecord} ondelete={deleteRecord} />
							{/if}

							{#snippet footer()}
								{#if panel?.kind === 'stage'}
									<button
										type="button"
										class="remove"
										onclick={() => {
											if (panel?.kind !== 'stage') return;
											const name = panel.name;
											change(removeStage(document, name));
											panel = null;
											announce(`Removed stage "${name}". Undo brings it back.`);
										}}
									>
										<Icon name="trash" size={16} /> Remove stage
									</button>
								{:else if panel?.kind === 'role'}
									<button
										type="button"
										class="remove"
										onclick={() => {
											if (panel?.kind !== 'role') return;
											const name = panel.name;
											const uses = roleUses(document.blueprint, name);
											change(removeRole(document, name));
											panel = null;
											announce(
												uses > 0
													? `Removed "${name}". ${uses} rule${uses === 1 ? '' : 's'} used it and now name nobody. Undo brings it back.`
													: `Removed "${name}".`
											);
										}}
									>
										<Icon name="trash" size={16} /> Remove role
									</button>
								{/if}
							{/snippet}
						</SidePanel>
					</div>
				{/if}
			</div>
		</div>
	</div>

	{#if compact}
		<StageList
			{document}
			problems={stageProblems}
			onopen={(name) => open({ kind: 'stage', name })}
		/>
	{/if}

	<p class="visually-hidden" aria-live="polite">{announcement}</p>
</div>

<style>
	.blueprint-editor {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 1rem;
	}

	.banner {
		display: flex;
		gap: 0.75rem;
		align-items: center;
		padding: 0.5rem 0.5rem 0.5rem 1rem;
		border: 1px solid var(--color-accent);
		color: var(--color-text);
	}

	.banner > :global(.icon) {
		color: var(--color-accent);
	}

	.banner p {
		flex: 1;
	}

	.frame {
		display: grid;
		flex: 1;
		grid-template-rows: auto minmax(0, 1fr);
		grid-template-columns: minmax(0, 1fr);
		height: 0;
		min-height: 34rem;
		border: var(--border-hairline);
	}

	.bar {
		position: relative;
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem 0.75rem;
		align-items: center;
		padding: 0.5rem 0.5rem 0.5rem 0.75rem;
		border-bottom: var(--border-hairline);
	}

	.name {
		display: flex;
		gap: 0.375rem;
		align-items: center;
		min-width: 12rem;
	}

	.name input {
		width: 16rem;
		border-color: transparent;
		background: transparent;
		font-weight: 500;
	}

	.name input:hover {
		border-color: var(--color-border);
	}

	.saved-mark {
		display: grid;
		place-items: center;
		width: 1.5rem;
		color: var(--color-text-faint);
		cursor: help;
	}

	.saved-mark.unsaved {
		color: var(--color-accent);
	}

	.tools {
		display: flex;
		gap: 0.125rem;
		align-items: center;
	}

	.tool {
		display: grid;
		place-items: center;
		width: 2.25rem;
		height: 2.25rem;
		min-height: 0;
		padding: 0;
		border: 1px solid transparent;
		background: transparent;
		color: var(--color-text-muted);
	}

	.tool:hover:not(:disabled) {
		border-color: var(--color-border);
		color: var(--color-text);
	}

	.tool[aria-pressed='true'] {
		border-color: var(--color-border);
		background: var(--color-surface);
		color: var(--color-text);
	}

	.tool:disabled {
		color: var(--color-text-faint);
		opacity: 0.5;
	}

	.tool:focus-visible {
		outline: 1px solid var(--color-accent);
		outline-offset: 1px;
	}

	.status {
		display: flex;
		gap: 0.25rem;
		width: auto;
		min-width: 2.25rem;
		padding: 0 0.5rem;
	}

	.status.attention {
		color: var(--color-accent);
	}

	.count {
		font: 500 0.75rem / 1 var(--font-mono);
	}

	.divider {
		width: 1px;
		height: 1.25rem;
		margin: 0 0.375rem;
		background: var(--color-border);
	}

	.spacer {
		flex: 1;
	}

	.toast {
		position: absolute;
		top: calc(100% + 0.5rem);
		left: 50%;
		z-index: 30;
		max-width: min(36rem, calc(100% - 2rem));
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--color-border);
		background: var(--color-surface);
		transform: translateX(-50%);
	}

	.body {
		display: grid;
		grid-template-columns: 19rem minmax(0, 1fr);
		min-height: 0;
	}

	.rail {
		overflow: auto;
		padding: 1.25rem 1rem 1.5rem 1.25rem;
		border-right: var(--border-hairline);
	}

	.canvas {
		position: relative;
		min-height: 0;
		overflow: hidden;
	}

	.canvas :global(.map) {
		border: 0;
	}

	.panel-host {
		position: absolute;
		inset: 0;
		overflow: hidden;
		pointer-events: none;
	}

	.panel-host > :global(*) {
		pointer-events: auto;
	}

	.panel-host.compact {
		position: fixed;
		z-index: 40;
	}

	.panel-host.compact :global(.panel) {
		width: 100%;
	}

	.remove {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.prompt {
		margin: 0;
		color: var(--color-text-muted);
		font: var(--text-code);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.faint {
		margin-bottom: 0.75rem;
		color: var(--color-text-subtle);
	}

	@media (max-width: 64rem) {
		.body {
			grid-template-columns: 16rem minmax(0, 1fr);
		}
	}

	.frame.compact {
		flex: none;
		height: auto;
		min-height: 0;
	}

	.frame.compact .body {
		grid-template-columns: 1fr;
	}

	.frame.compact .rail {
		border-right: 0;
		border-bottom: var(--border-hairline);
	}

	.frame.compact .canvas {
		height: 22rem;
	}

	.frame.compact .name {
		flex: 1 1 100%;
	}

	.frame.compact .name input {
		width: 100%;
	}

	.frame.compact .bar :global(.copy) {
		flex: 1 1 100%;
	}
</style>
