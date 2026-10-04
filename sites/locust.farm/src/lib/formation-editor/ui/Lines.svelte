<!--
	A compact rules matrix. The questions share one header on wide screens and
	repeat beside each answer on phones. Every cell opens the existing choices.
-->
<script lang="ts">
	import type { EditorDocument } from '../model/document.ts';
	import {
		renameStage,
		renameTaskType,
		removeStage,
		setTaskType,
		stageOrder,
		usableName
	} from '../model/edit.ts';
	import {
		addKind,
		addStep,
		follows,
		kinds,
		MAIN,
		type LineRef,
		type PointName
	} from '../model/line.ts';
	import { phrase, QUESTIONS } from '../model/words.ts';
	import Icon from './Icon.svelte';
	import PointBox from './PointBox.svelte';
	import { pointKey } from './problems.ts';

	let {
		document,
		open,
		problems,
		onchange,
		onopen,
		onannounce
	}: {
		document: EditorDocument;
		/** The point whose choices are shown, if any. */
		open: { line: LineRef; point: PointName } | null;
		/** The points that have a problem, by pointKey. */
		problems: Set<string>;
		onchange: (next: EditorDocument) => void;
		onopen: (next: { line: LineRef; point: PointName } | null) => void;
		onannounce: (message: string) => void;
	} = $props();

	const POINTS: PointName[] = ['add', 'work', 'counts', 'pick'];

	const formation = $derived(document.formation);
	const steps = $derived(stageOrder(formation));
	const otherKinds = $derived(kinds(formation));
	const openKey = $derived(open ? pointKey(open.line, open.point) : null);

	function toggle(line: LineRef, point: PointName) {
		onopen(openKey === pointKey(line, point) ? null : { line, point });
	}

	function rename(event: Event, line: Exclude<LineRef, { kind: 'main' }>) {
		const input = event.target as HTMLInputElement;
		const to = input.value.trim();
		if (to === line.name) return;
		const what = line.kind === 'step' ? 'step' : 'kind of task';
		const taken = Object.hasOwn(formation.flow, to) || Object.hasOwn(formation.task_types, to);
		if (!usableName(to) || taken) {
			input.value = line.name;
			onannounce(
				to === ''
					? `A ${what} needs a name.`
					: taken
						? `The name "${to}" is already used.`
						: 'That name cannot be used.'
			);
			return;
		}
		const next =
			line.kind === 'step'
				? renameStage(document, line.name, to)
				: renameTaskType(document, line.name, to);
		if (open && open.line.kind === line.kind && open.line.name === line.name) {
			onopen({ line: { kind: line.kind, name: to }, point: open.point });
		}
		onchange(next);
	}

	function remove(line: Exclude<LineRef, { kind: 'main' }>) {
		if (open && open.line.kind === line.kind && open.line.name === line.name) onopen(null);
		onchange(
			line.kind === 'step'
				? removeStage(document, line.name)
				: setTaskType(document, line.name, null)
		);
		onannounce(
			`Removed the ${line.kind === 'step' ? 'step' : 'kind of task'} "${line.name}". Undo brings it back.`
		);
	}

	function newStep() {
		const added = addStep(document);
		onchange(added.document);
		onannounce(`Added the step "${added.name}". Give it a name.`);
	}

	function newKind() {
		const added = addKind(document);
		onchange(added.document);
		onannounce(`Added the kind of task "${added.name}". Give it a name.`);
	}
</script>

{#snippet box(line: LineRef, point: PointName)}
	<div class="box-host">
		{#key pointKey(line, point)}
			<PointBox {document} {line} {point} {onchange} onclose={() => onopen(null)} />
		{/key}
	</div>
{/snippet}

{#snippet row(line: Exclude<LineRef, { kind: 'main' }>)}
	<div class="line row" data-name={line.name}>
		<input
			class="name"
			type="text"
			value={line.name}
			aria-label={line.kind === 'step' ? 'Name of this step' : 'Name of this kind of task'}
			onchange={(event) => rename(event, line)}
		/>
		{#each POINTS as point (point)}
			{@const key = pointKey(line, point)}
			{@const same = point !== 'add' && follows(document, line, point)}
			{#if point === 'add' && line.kind === 'kind'}
				<p class="cell fixed">
					<span class="question">{QUESTIONS[point]}</span>
					<span class="answer">{phrase(formation, line, point)}</span>
				</p>
			{:else}
				<button
					type="button"
					class="cell"
					class:same
					class:open={openKey === key}
					class:problem={problems.has(key)}
					data-point={point}
					data-key={key}
					aria-expanded={openKey === key}
					onclick={() => toggle(line, point)}
				>
					<span class="question">{QUESTIONS[point]}</span>
					<span class="answer">{same ? 'Same as any task' : phrase(formation, line, point)}</span>
				</button>
			{/if}
		{/each}
		<button
			type="button"
			class="icon-button remove"
			aria-label={`Remove the ${line.kind === 'step' ? 'step' : 'kind of task'} "${line.name}"`}
			onclick={() => remove(line)}
		>
			<Icon name="x" size={16} />
		</button>
		{#if open && open.line.kind === line.kind && open.line.name === line.name}
			{@render box(line, open.point)}
		{/if}
	</div>
{/snippet}

<div class="lines">
	<div class="column-headings" aria-hidden="true">
		<span></span>
		{#each POINTS as point (point)}<span class="question">{QUESTIONS[point]}</span>{/each}
		<span></span>
	</div>
	<section class="line main" aria-labelledby="any-task-title">
		<div class="lead">
			<h2 id="any-task-title">Any task</h2>
			<p class="row-label">Default rules</p>
		</div>
		{#each POINTS as point (point)}
			{@const key = pointKey(MAIN, point)}
			<button
				type="button"
				class="point"
				class:open={openKey === key}
				class:problem={problems.has(key)}
				data-point={point}
				data-key={key}
				aria-expanded={openKey === key}
				onclick={() => toggle(MAIN, point)}
			>
				<span class="question">{QUESTIONS[point]}</span>
				<span class="answer">{phrase(formation, MAIN, point)}</span>
				<span class="change">Change</span>
			</button>
		{/each}
		<span class="end" aria-hidden="true"></span>
		{#if open && open.line.kind === 'main'}
			{@render box(MAIN, open.point)}
		{/if}
	</section>

	{#if steps.length > 0}
		<section class="group" aria-labelledby="steps-title">
			<header>
				<h2 id="steps-title">Steps</h2>
			</header>
			{#each steps as name (name)}
				{@render row({ kind: 'step', name })}
			{/each}
		</section>
	{/if}

	{#if otherKinds.length > 0}
		<section class="group" aria-labelledby="kinds-title">
			<header>
				<h2 id="kinds-title">Other kinds of task</h2>
			</header>
			{#each otherKinds as name (name)}
				{@render row({ kind: 'kind', name })}
			{/each}
		</section>
	{/if}

	<div class="adders">
		<button type="button" onclick={newStep} aria-describedby="step-help"
			><Icon name="plus" size={14} /> Step</button
		>
		<button type="button" onclick={newKind} aria-describedby="kind-help">
			<Icon name="plus" size={14} /> Another kind of task
		</button>
		<details class="task-help">
			<summary>About tasks</summary>
			<div>
				<p id="step-help">
					<strong>Step.</strong> One task that Locust adds, after the step before it. Each step runs once
					for the whole goal, not once per task. It follows the rules for any task unless you change a
					point.
				</p>
				<p id="kind-help"><strong>Another kind of task.</strong> Tasks that follow other rules.</p>
			</div>
		</details>
	</div>
</div>

<style>
	.lines {
		--columns: 10rem repeat(4, minmax(0, 1fr)) 2.75rem;
		display: grid;
	}
	.line,
	.column-headings {
		display: grid;
		grid-template-columns: var(--columns);
		gap: 0;
		border-bottom: var(--border-hairline);
	}
	.column-headings > span {
		padding: 0.875rem 1rem;
	}
	.column-headings > span + span {
		border-left: var(--border-hairline);
	}
	.group {
		gap: 0;
	}
	h2 {
		color: var(--color-text);
	}
	.lead {
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 0.25rem;
		padding: 1rem 1.25rem;
	}
	.row-label {
		margin: 0;
		color: var(--color-text-subtle);
		font: var(--text-label);
	}
	.point,
	.cell {
		position: relative;
		display: grid;
		align-content: center;
		gap: 0.375rem;
		min-width: 0;
		min-height: 5rem;
		padding: 1rem;
		border: 0;
		border-left: var(--border-hairline);
		background: transparent;
		color: var(--color-text);
		font: var(--text-ui);
		text-align: left;
	}
	button.point,
	button.cell {
		cursor: pointer;
	}
	button.point:hover,
	button.cell:hover {
		background: color-mix(in srgb, var(--color-surface) 55%, var(--color-bg));
	}
	.point.open,
	.cell.open {
		background: var(--color-surface);
		box-shadow: inset 0 -2px 0 var(--color-accent);
	}
	.point.problem,
	.cell.problem {
		box-shadow: inset 0 0 0 1px var(--color-accent);
	}
	.question {
		color: var(--color-text-subtle);
		font: var(--text-label);
		letter-spacing: var(--tracking-label);
		text-transform: uppercase;
	}
	.line .question {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}
	.answer {
		text-wrap: pretty;
		overflow-wrap: anywhere;
	}

	.change {
		justify-self: start;
		color: var(--color-text-subtle);
		font: var(--text-label);
		letter-spacing: var(--tracking-label);
		text-decoration: underline;
		text-transform: uppercase;
	}

	.point:hover .change,
	.point.open .change {
		color: var(--color-accent);
	}

	.end {
		border-left: var(--border-hairline);
	}
	.box-host {
		grid-column: 1 / -1;
	}
	.group > header {
		padding: 0.5rem 1.25rem;
		border-bottom: var(--border-hairline);
		background: color-mix(in srgb, var(--color-surface) 25%, var(--color-bg));
	}
	.group > header h2 {
		color: var(--color-text-subtle);
		font: var(--text-label);
	}
	.row .name {
		align-self: center;
		min-width: 0;
		width: auto;
		margin: 0.625rem 0.75rem 0.625rem 1.25rem;
		padding-inline: 0;
		border: 0;
		border-bottom: 1px dashed var(--color-border);
		background: transparent;
		font-weight: 500;
	}
	.cell.same .answer {
		color: var(--color-text-subtle);
		font-size: 0.75rem;
	}
	.cell.fixed {
		margin: 0;
		color: var(--color-text-muted);
	}
	.remove {
		align-self: center;
		margin: 0.5rem 0.25rem;
		border: 0;
		background: transparent;
	}
	.adders {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem 0.75rem;
		align-items: center;
		padding: 0.875rem 1.25rem;
		border-bottom: var(--border-hairline);
	}
	.adders button {
		display: flex;
		gap: 0.375rem;
		align-items: center;
		background: color-mix(in srgb, var(--color-surface) 45%, var(--color-bg));
		font-size: 0.75rem;
	}
	.task-help {
		color: var(--color-text-subtle);
		font-size: 0.75rem;
	}
	.task-help[open] {
		flex-basis: 100%;
	}
	.task-help summary {
		width: fit-content;
		padding: 0.5rem 0;
		cursor: pointer;
	}
	.task-help summary:focus-visible {
		outline: 1px solid var(--color-accent);
		outline-offset: 2px;
	}
	.task-help p {
		margin: 0.375rem 0;
	}
	@media (max-width: 64rem) {
		.lines {
			--columns: 8rem repeat(4, minmax(0, 1fr)) 2.75rem;
		}
	}
	@media (max-width: 48rem) {
		.column-headings {
			display: none;
		}
		.line.main,
		.line.row {
			grid-template-columns: minmax(0, 1fr) 2.75rem;
		}
		.lead {
			grid-column: 1 / -1;
			padding: 1rem;
			border-bottom: var(--border-hairline);
		}
		.line .point,
		.row .cell {
			grid-column: 1 / -1;
			min-height: 0;
			border-left: 0;
			padding: 0.875rem 1rem;
		}
		.line .question {
			position: static;
			width: auto;
			height: auto;
			overflow: visible;
			clip-path: none;
		}
		.end {
			display: none;
		}
		.row > .remove {
			grid-row: 1;
			grid-column: 2;
		}
		.adders {
			padding-inline: 1rem;
		}
	}
</style>
