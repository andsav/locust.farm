<!--
	The formation editor: the ways of working, one toolbar, the rules as lines of
	four points, the roles, and a side panel for everything else.
-->
<script lang="ts">
	import './editor.css';
	import { onMount, tick } from 'svelte';
	import { inspectFormation } from '../contract/inspect.ts';
	import { pageChecks } from '../model/checks.ts';
	import { formationText, type EditorDocument } from '../model/document.ts';
	import {
		addRole,
		applyWay,
		newDocument,
		removeRole,
		roleUses,
		setName,
		stageOrder,
		usableName
	} from '../model/edit.ts';
	import { record, redo, startHistory, undo, type History } from '../model/history.ts';
	import type { LineRef, PointName } from '../model/line.ts';
	import { matchingWay, wayOfWorking } from '../model/presets.ts';
	import { summarize } from '../model/words.ts';
	import { openText } from '../prompt/open.ts';
	import {
		blocks,
		buildPrompt,
		draftId,
		LARGE_PROMPT,
		type BuiltPrompt
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
	import Lines from './Lines.svelte';
	import OpenFormation from './OpenFormation.svelte';
	import Problems from './Problems.svelte';
	import {
		fromDiagnostics,
		fromPageChecks,
		pointKey,
		problemPoints,
		type Place
	} from './problems.ts';
	import RoleSettings from './RoleSettings.svelte';
	import SavedList from './SavedList.svelte';
	import SidePanel from './SidePanel.svelte';
	import Summary from './Summary.svelte';
	import { tip } from './tooltip.ts';
	import WayPicker from './WayPicker.svelte';

	type Panel =
		{ kind: 'role'; name: string } | { kind: 'problems' | 'words' | 'prompt' | 'open' | 'saved' };

	let history = $state.raw<History>(startHistory(newDocument()));
	const document = $derived(history.present);

	let panel = $state<Panel | null>(null);
	/** The point of a line whose choices are shown, if any. */
	let openPoint = $state<{ line: LineRef; point: PointName } | null>(null);
	let announcement = $state('');
	let saveStatus = $state<{ ok: boolean; text: string }>({ ok: true, text: '' });
	let banner = $state<string | null>(null);
	let compact = $state(false);

	let store: KeyValueStore | null = null;
	let recordId = $state(newRecordId());
	let origin: 'new' | 'link' | 'import' = 'new';

	const inspection = $derived(inspectFormation(document.formation));
	const checks = $derived(pageChecks(document.formation));
	const problems = $derived([
		...fromDiagnostics(inspection.diagnostics, document.formation),
		...fromPageChecks(checks, document.formation)
	]);
	const marked = $derived(problemPoints(problems));
	const blocked = $derived(
		checks.some((check) => check.blocksCopy)
			? 'Remove the invisible characters listed under problems before copying.'
			: null
	);
	const lines = $derived(summarize(document.formation));
	const kept = $derived(lines.filter((line) => line.area === 'kept'));
	const way = $derived(matchingWay(document.formation));
	const roles = $derived(Object.keys(document.formation.roles));
	const hasSteps = $derived(stageOrder(document.formation).length > 0);

	// The prompt follows every change.
	let prompt = $state<BuiltPrompt | null>(null);
	$effect(() => {
		const current = { document, hasErrors: inspection.diagnostics.length > 0 };
		let cancelled = false;
		buildPrompt(current.document, 'add', current.hasErrors).then((built) => {
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

	/** Closes a panel or a point whose role, step or kind no longer exists. */
	function keepOpenValid() {
		const formation = history.present.formation;
		if (panel?.kind === 'role' && !Object.hasOwn(formation.roles, panel.name)) panel = null;
		const line = openPoint?.line;
		if (line?.kind === 'step' && !Object.hasOwn(formation.flow, line.name)) openPoint = null;
		if (line?.kind === 'kind' && !Object.hasOwn(formation.task_types, line.name)) openPoint = null;
	}

	function pickWay(id: string) {
		change(applyWay(document, id));
		panel = null;
		openPoint = null;
		announce(`Loaded the ${wayOfWorking(id)?.title ?? id} way of working.`);
	}

	function toggle(kind: 'problems' | 'words' | 'prompt' | 'open' | 'saved') {
		fileOpen = false;
		if (panel?.kind === kind) {
			panel = null;
			return;
		}
		if (kind === 'saved') saved = store ? listSaved(store) : [];
		if (kind === 'open') openMessage = '';
		panel = { kind };
	}

	async function show(place: Place) {
		panel = null;
		if (place.kind === 'roles') {
			openPoint = null;
			await tick();
			window.document.getElementById('section-roles')?.scrollIntoView({ block: 'center' });
			return;
		}
		openPoint = { line: place.ref, point: place.point };
		await tick();
		const key = pointKey(place.ref, place.point);
		const control = window.document.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`);
		control?.scrollIntoView({ block: 'center' });
		control?.focus();
	}

	// Roles.
	let naming = $state(false);
	let newRole = $state('');

	function addRoleNamed() {
		const name = newRole.trim();
		naming = false;
		newRole = '';
		if (name === '') return;
		if (!usableName(name)) {
			toast('That name cannot be used.');
			return;
		}
		if (Object.hasOwn(document.formation.roles, name)) {
			toast(`There is already a role called "${name}".`);
			return;
		}
		change(addRole(document, name));
		announce(`Added the role "${name}".`);
	}

	function focus(element: HTMLInputElement) {
		element.focus();
	}

	// The File menu.
	let fileOpen = $state(false);
	let fileMenu: HTMLDivElement | undefined = $state();

	function onpointerdown(event: PointerEvent) {
		if (fileOpen && fileMenu && !fileMenu.contains(event.target as Node)) fileOpen = false;
	}

	function doUndo() {
		history = undo(history);
		keepOpenValid();
		scheduleSave();
		announce('Undone.');
	}

	function doRedo() {
		history = redo(history);
		keepOpenValid();
		scheduleSave();
		announce('Redone.');
	}

	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			if (fileOpen) fileOpen = false;
			else if (panel) panel = null;
			else if (openPoint) openPoint = null;
			return;
		}
		// Text fields keep their own undo; a ticked box or a chosen option does not.
		const field = (event.target as HTMLElement).closest('input, textarea, [contenteditable]');
		const typing =
			field !== null &&
			!(field instanceof HTMLInputElement && ['radio', 'checkbox'].includes(field.type));
		if (typing) return;
		const mod = event.metaKey || event.ctrlKey;
		if (mod && event.key.toLowerCase() === 'z') {
			event.preventDefault();
			if (event.shiftKey) doRedo();
			else doUndo();
		} else if (mod && event.key.toLowerCase() === 'y') {
			event.preventDefault();
			doRedo();
		}
	}

	function load(next: EditorDocument, from: 'new' | 'link' | 'import', id = newRecordId()) {
		history = startHistory(next);
		recordId = id;
		origin = from;
		panel = null;
		openPoint = null;
	}

	// Saved in this browser.
	let saved = $state<SavedSummary[]>([]);

	function openRecord(id: string) {
		if (!store) return;
		const found = loadSaved(store, id);
		const opened = found ? openText(found.data) : null;
		if (found && opened?.ok) {
			load(opened.document, found.origin, found.id);
			announce(`Opened "${found.name || 'Untitled formation'}".`);
		}
	}

	function deleteRecord(id: string) {
		if (!store) return;
		removeSaved(store, id);
		saved = listSaved(store);
		if (id === recordId) recordId = newRecordId();
	}

	// Open a formation.
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
			opened.note ?? 'Opened the formation. Your earlier work is still under Saved in this browser.'
		);
	}

	// Links and downloads.
	async function copyLink() {
		fileOpen = false;
		const data = await blocks(document);
		const fragment = await encodeLink(data.text);
		const url = `${location.origin}${location.pathname}#${fragment}`;
		const result = await copyText(url, navigator.clipboard);
		const privacy =
			'Anyone with the link can read this formation, and it stays in browser history.';
		toast(
			result === 'copied'
				? `Link copied. ${privacy}${url.length > LONG_LINK ? ' This link is long; some chat apps may cut it, so Download may work better.' : ''}`
				: 'Copying the link did not work. Use Download instead.'
		);
	}

	function download() {
		fileOpen = false;
		const blob = new Blob([formationText(document.formation)], { type: 'application/json' });
		const link = window.document.createElement('a');
		link.href = URL.createObjectURL(blob);
		link.download = `${draftId(document.name)}.json`;
		link.click();
		setTimeout(() => URL.revokeObjectURL(link.href), 1000);
	}

	function startOver() {
		fileOpen = false;
		load(newDocument(), 'new');
		scheduleSave();
		toast('Started a new formation. The earlier one is under Saved in this browser.');
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

	/** Copies a prompt that only has the agent check the formation and explain it. */
	async function copyCheckOnly() {
		const built = await buildPrompt(document, 'check', inspection.diagnostics.length > 0);
		const result = await copyText(built.text, navigator.clipboard);
		toast(
			result === 'copied'
				? 'Copied a prompt that only checks. Your agent checks the formation with Locust and explains it. Nothing is saved.'
				: 'Copying did not work. Use "See the prompt" and copy it by hand.'
		);
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

	const panelTitle = $derived.by((): { title: string; mark: IconName } | null => {
		if (!panel) return null;
		switch (panel.kind) {
			case 'role':
				return { title: panel.name, mark: 'user' };
			case 'problems':
				return {
					title:
						problems.length === 0
							? 'No problems'
							: problems.length === 1
								? '1 problem'
								: `${problems.length} problems`,
					mark: problems.length === 0 ? 'check-circle' : 'warning'
				};
			case 'words':
				return { title: 'In words', mark: 'article' };
			case 'prompt':
				return {
					title: prompt
						? `The prompt (${prompt.text.length.toLocaleString('en')} characters)`
						: 'The prompt',
					mark: 'eye'
				};
			case 'open':
				return { title: 'Open a formation', mark: 'folder-open' };
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
							'Opened from a link. If someone else made it, read its names before you copy it.';
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

<svelte:window {onkeydown} {onpointerdown} />

{#snippet removeRoleButton()}
	<button
		type="button"
		class="remove"
		onclick={() => {
			if (panel?.kind !== 'role') return;
			const name = panel.name;
			const uses = roleUses(document.formation, name);
			change(removeRole(document, name));
			panel = null;
			toast(
				uses > 0
					? `Removed "${name}". ${uses} rule${uses === 1 ? '' : 's'} used it and now name nobody. Undo brings it back.`
					: `Removed "${name}".`
			);
		}}
	>
		<Icon name="trash" size={16} /> Remove role
	</button>
{/snippet}

<div class="formation-editor">
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

	<WayPicker current={way} onpick={pickWay} />

	<div class="frame">
		<div class="bar">
			<div class="name">
				<input
					type="text"
					value={document.name}
					aria-label="Name of this formation"
					placeholder="Untitled formation"
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

			<div class="tools" role="toolbar" aria-label="Formation">
				<button
					type="button"
					class="tool"
					aria-label="Undo"
					disabled={history.past.length === 0}
					use:tip={{ label: 'Undo', description: 'Cmd or Ctrl + Z' }}
					onclick={doUndo}
				>
					<Icon name="arrow-counter-clockwise" />
				</button>
				<button
					type="button"
					class="tool"
					aria-label="Redo"
					disabled={history.future.length === 0}
					use:tip={{ label: 'Redo', description: 'Shift + Cmd or Ctrl + Z' }}
					onclick={doRedo}
				>
					<Icon name="arrow-clockwise" />
				</button>
				<div class="file" bind:this={fileMenu}>
					<button
						type="button"
						class="word"
						aria-haspopup="menu"
						aria-expanded={fileOpen}
						onclick={() => (fileOpen = !fileOpen)}
					>
						File
					</button>
					{#if fileOpen}
						<div class="menu" role="menu">
							<button type="button" role="menuitem" onclick={() => toggle('open')}>
								Open a formation
							</button>
							<button type="button" role="menuitem" onclick={() => toggle('saved')}>
								Open one saved in this browser
							</button>
							<button type="button" role="menuitem" onclick={copyLink}>Copy a link to it</button>
							<button type="button" role="menuitem" onclick={download}>Download as JSON</button>
							<button type="button" role="menuitem" onclick={startOver}>Start over</button>
						</div>
					{/if}
				</div>
			</div>

			<span class="spacer"></span>

			<button
				type="button"
				class="word status"
				class:attention={problems.length > 0}
				aria-pressed={panel?.kind === 'problems'}
				onclick={() => toggle('problems')}
			>
				<Icon name={problems.length === 0 ? 'check-circle' : 'warning'} size={16} />
				{problems.length === 0
					? 'No problems'
					: problems.length === 1
						? '1 problem'
						: `${problems.length} problems`}
			</button>
			<button
				type="button"
				class="word"
				aria-pressed={panel?.kind === 'words'}
				onclick={() => toggle('words')}
			>
				<Icon name="article" size={16} /> In words
			</button>

			<CopyBar {prompt} {blocked} onfail={copyFailed} />

			<p class="next">
				Paste it into one coding agent. It saves the formation in the Locust on that computer as a
				private draft and asks before publishing. Starting a goal comes later.
				<button type="button" class="quiet" onclick={() => toggle('prompt')}>See the prompt</button>
				<button type="button" class="quiet" onclick={copyCheckOnly}>
					Copy a prompt that checks it and saves nothing
				</button>
				<a href="/start">Set up Locust</a>
				<a href="/docs/next/formation-authoring">Manual</a>
			</p>

			{#if toastText}
				<p class="toast">{toastText}</p>
			{/if}
		</div>

		<div class="body">
			<Lines
				{document}
				open={openPoint}
				problems={marked}
				onchange={change}
				onopen={(next) => (openPoint = next)}
				onannounce={announce}
			/>

			<section id="section-roles" class="roles" aria-labelledby="roles-title">
				<h2 id="roles-title">Roles</h2>
				<ul class="chips">
					{#each roles as role (role)}
						<li>
							<button
								type="button"
								class="chip"
								class:open={panel?.kind === 'role' && panel.name === role}
								aria-label={`Role "${role}"`}
								onclick={() => (panel = { kind: 'role', name: role })}
							>
								<Icon name="user" size={14} />
								<span>{role}</span>
							</button>
						</li>
					{/each}
					<li>
						{#if naming}
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
									aria-label="Name of the new role"
									placeholder="reviewer"
									use:focus
									onblur={addRoleNamed}
									onkeydown={(event) => {
										if (event.key === 'Escape') {
											event.stopPropagation();
											newRole = '';
											naming = false;
										}
									}}
								/>
							</form>
						{:else}
							<button type="button" class="chip add" onclick={() => (naming = true)}>
								<Icon name="plus" size={14} /> Role
							</button>
						{/if}
					</li>
				</ul>
				<p class="muted">
					{roles.length === 0
						? 'None. Every member takes part on equal terms.'
						: 'Members are put into roles later, in Locust.'}
				</p>
			</section>

			<div class="notes">
				{#each kept as line, index (index)}
					<p>{line.text}</p>
				{/each}
				<p>
					A member is one agent or one person. Locust applies these rules to what members add and
					post. It does not start agents or run checks.
				</p>
			</div>

			{#if panel && panelTitle}
				<div class="panel-host" class:compact>
					<SidePanel
						open
						mark={panelTitle.mark}
						title={panelTitle.title}
						footer={panel.kind === 'role' ? removeRoleButton : undefined}
						onclose={() => (panel = null)}
					>
						{#if panel.kind === 'role' && Object.hasOwn(document.formation.roles, panel.name)}
							<RoleSettings
								{document}
								name={panel.name}
								onchange={change}
								onrename={(to) => (panel = { kind: 'role', name: to })}
								onannounce={announce}
							/>
						{:else if panel.kind === 'problems'}
							<Problems {problems} onshow={show} />
						{:else if panel.kind === 'words'}
							<Summary
								lines={lines.filter((line) => line.area !== 'kept')}
								explanation={inspection.explanation}
								{hasSteps}
							/>
						{:else if panel.kind === 'prompt'}
							{#if prompt && prompt.text.length > LARGE_PROMPT}
								<p class="faint">
									This prompt is long. Some agents shorten very long pastes; if yours does, download
									the formation and give your agent the file.
								</p>
							{/if}
							<pre class="prompt" bind:this={promptElement}>{prompt?.text ?? ''}</pre>
						{:else if panel.kind === 'open'}
							<OpenFormation message={openMessage} onopen={openFrom} />
						{:else if panel.kind === 'saved'}
							<SavedList {saved} current={recordId} onopen={openRecord} ondelete={deleteRecord} />
						{/if}
					</SidePanel>
				</div>
			{/if}
		</div>
	</div>

	<p class="visually-hidden" aria-live="polite">{announcement}</p>
</div>

<style>
	.formation-editor {
		display: flex;
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
		border: var(--border-hairline);
	}

	.bar {
		position: relative;
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem 0.5rem;
		align-items: center;
		padding: 0.5rem 0.5rem 0.625rem 0.75rem;
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

	.tool:disabled {
		color: var(--color-text-faint);
		opacity: 0.5;
	}

	.word {
		display: flex;
		gap: 0.375rem;
		align-items: center;
		border-color: transparent;
		background: transparent;
		color: var(--color-text-muted);
	}

	.word:hover {
		border-color: var(--color-border);
		color: var(--color-text);
	}

	.word[aria-pressed='true'],
	.word[aria-expanded='true'] {
		border-color: var(--color-border);
		background: var(--color-surface);
		color: var(--color-text);
	}

	.status.attention {
		color: var(--color-accent);
	}

	.file {
		position: relative;
	}

	.menu {
		position: absolute;
		top: calc(100% + 0.25rem);
		left: 0;
		z-index: 30;
		display: grid;
		min-width: 14rem;
		border: var(--border-hairline);
		background: var(--color-surface);
		box-shadow: 0 12px 28px -12px rgb(0 0 0 / 0.9);
	}

	.menu button {
		border: 0;
		background: transparent;
		text-align: left;
	}

	.menu button:hover {
		background: var(--color-bg);
	}

	.spacer {
		flex: 1;
	}

	.next {
		display: flex;
		flex: 1 1 100%;
		flex-wrap: wrap;
		gap: 0.125rem 0.875rem;
		justify-content: flex-end;
		align-items: baseline;
		margin: 0;
		color: var(--color-text-subtle);
		font: var(--text-label);
	}

	.next button,
	.next a {
		min-height: 0;
		padding: 0;
		color: var(--color-text-muted);
		font: inherit;
		text-decoration: underline;
	}

	/* At the foot of the window, so it never covers the questions. */
	.toast {
		position: fixed;
		bottom: 1.5rem;
		left: 50%;
		z-index: 50;
		max-width: min(36rem, calc(100% - 2rem));
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--color-border);
		background: var(--color-surface);
		transform: translateX(-50%);
	}

	.body {
		position: relative;
		min-height: 24rem;
	}

	.roles {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem 1rem;
		align-items: center;
		padding: 0.875rem 1.25rem;
		border-bottom: var(--border-hairline);
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
		background: transparent;
		color: var(--color-text-muted);
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

	.muted {
		margin: 0;
		color: var(--color-text-subtle);
	}

	.notes {
		display: grid;
		gap: 0.25rem;
		padding: 0.875rem 1.25rem 1rem;
		color: var(--color-text-subtle);
	}

	.notes p {
		margin: 0;
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

	@media (max-width: 48rem) {
		.name {
			flex: 1 1 100%;
		}

		.name input {
			width: 100%;
		}

		.bar :global(.copy) {
			flex: 1 1 100%;
		}

		.next {
			justify-content: flex-start;
		}
	}
</style>
