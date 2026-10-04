<!--
	The rules as lines of four points: who adds a task, who works on it, when a
	result counts and whether one result is picked. The first line is the rules
	for any task, with a picture at each point. Steps and other kinds of task are
	shorter lines under it that say only what differs.
-->
<script lang="ts">
	import type { EditorDocument } from '../model/document.ts';
	import {
		renameStage,
		renameTaskType,
		removeStage,
		setTaskType,
		stageOrder
	} from '../model/edit.ts';
	import {
		addAnswer,
		addKind,
		addStep,
		countsAnswer,
		kinds,
		lineRules,
		MAIN,
		pickAnswer,
		sameAsMain,
		workAnswer,
		type LineRef,
		type PointName
	} from '../model/line.ts';
	import { phrase, QUESTIONS } from '../model/words.ts';
	import { drawLine, type PointPicture } from './diagrams.ts';
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

	const blueprint = $derived(document.blueprint);
	const steps = $derived(stageOrder(blueprint));
	const otherKinds = $derived(kinds(blueprint));
	const openKey = $derived(open ? pointKey(open.line, open.point) : null);

	// The pictures follow the rules for any task.
	const pictures = $derived.by((): PointPicture[] => {
		const rules = lineRules(blueprint, MAIN);
		const add = addAnswer(rules.work);
		const work = workAnswer(rules.work);
		const counts = countsAnswer(rules.decisions.completion);
		const pick = pickAnswer(rules.decisions);
		return [
			{ point: 'add', kind: add.kind, role: add.kind === 'role' ? add.name : undefined },
			{
				point: 'work',
				kind: work.kind,
				role:
					work.kind === 'role'
						? work.name
						: work.kind === 'asks' && work.by.kind === 'role'
							? work.by.name
							: undefined
			},
			counts.kind === 'list'
				? {
						point: 'counts',
						approvals: counts.approvals?.count ?? 0,
						reviewers: counts.approvals?.by.kind === 'role' ? counts.approvals.by.name : undefined,
						check: counts.check?.name
					}
				: { point: 'counts', approvals: 0, own: true },
			{
				point: 'pick',
				picks: rules.decisions.selection !== null,
				role: pick.pick.kind === 'role' ? pick.pick.name : undefined
			}
		];
	});
	const drawn = $derived(JSON.stringify(pictures));
	const svgs: SVGSVGElement[] = $state([]);

	$effect(() => {
		const list = JSON.parse(drawn) as PointPicture[];
		if (svgs.length === POINTS.length && svgs.every(Boolean)) drawLine(svgs, list);
	});

	function toggle(line: LineRef, point: PointName) {
		onopen(openKey === pointKey(line, point) ? null : { line, point });
	}

	function rename(event: Event, line: Exclude<LineRef, { kind: 'main' }>) {
		const input = event.target as HTMLInputElement;
		const to = input.value.trim();
		if (to === line.name) return;
		const what = line.kind === 'step' ? 'step' : 'kind of task';
		const taken = Object.hasOwn(blueprint.flow, to) || Object.hasOwn(blueprint.task_types, to);
		if (to === '' || taken) {
			input.value = line.name;
			onannounce(to === '' ? `A ${what} needs a name.` : `The name "${to}" is already used.`);
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
			{@const same = point !== 'add' && sameAsMain(blueprint, line, point)}
			{#if point === 'add' && line.kind === 'kind'}
				<p class="cell fixed">
					<span class="question">{QUESTIONS[point]}</span>
					<span class="answer">{phrase(blueprint, line, point)}</span>
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
					<span class="answer">{same ? 'Same as any task' : phrase(blueprint, line, point)}</span>
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
	<section class="line main" aria-labelledby="any-task-title">
		<div class="lead">
			<h2 id="any-task-title">Any task</h2>
			<p class="muted">The rules every task follows.</p>
		</div>
		{#each POINTS as point, index (point)}
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
				<svg bind:this={svgs[index]} class="diagram" viewBox="0 0 240 120" aria-hidden="true"></svg>
				<span class="answer">{phrase(blueprint, MAIN, point)}</span>
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
				<p class="muted">
					Tasks Locust adds in order. A step follows the rules for any task unless you change a
					point.
				</p>
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
				<p class="muted">
					A member can add a task as one of these kinds. It then follows these rules instead.
				</p>
			</header>
			{#each otherKinds as name (name)}
				{@render row({ kind: 'kind', name })}
			{/each}
		</section>
	{/if}

	<div class="adders">
		<button type="button" onclick={newStep}><Icon name="plus" size={14} /> Step</button>
		<span class="muted">A task Locust adds, after the step before it.</span>
		<button type="button" onclick={newKind}>
			<Icon name="plus" size={14} /> Another kind of task
		</button>
		<span class="muted">Tasks that follow other rules.</span>
	</div>
</div>

<style>
	.lines {
		--columns: 10rem repeat(4, minmax(0, 1fr)) 2.75rem;

		display: grid;
	}

	.line {
		display: grid;
		grid-template-columns: var(--columns);
		gap: 0;
		border-bottom: var(--border-hairline);
	}

	.group {
		gap: 0;
	}

	/* A small arrowhead on the line between two points: a task moves left to right. */
	.main .point + .point::before {
		position: absolute;
		top: 6.25rem;
		left: -0.3125rem;
		width: 0.5rem;
		height: 0.5rem;
		border-top: 1px solid var(--color-text-faint);
		border-right: 1px solid var(--color-text-faint);
		background: var(--color-bg);
		content: '';
		transform: rotate(45deg);
	}

	h2 {
		color: var(--color-text);
	}

	.lead {
		display: grid;
		align-content: start;
		gap: 0.25rem;
		padding: 1rem 0.75rem 1rem 1.25rem;
	}

	.muted {
		margin: 0;
		color: var(--color-text-subtle);
	}

	.point,
	.cell {
		position: relative;
		display: grid;
		align-content: start;
		gap: 0.375rem;
		min-height: 0;
		padding: 0.875rem 1rem 1rem;
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
		box-shadow: inset 0 -1px 0 var(--color-accent);
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

	.diagram {
		display: block;
		width: 100%;
		max-width: 20rem;
		height: auto;
		margin: 0 auto;
	}

	.answer {
		text-wrap: pretty;
	}

	.end {
		border-left: var(--border-hairline);
	}

	.box-host {
		grid-column: 1 / -1;
	}

	.group > header {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem 1rem;
		align-items: baseline;
		padding: 1rem 1.25rem 0.625rem;
	}

	.row .name {
		align-self: start;
		width: auto;
		margin: 0.625rem 0.5rem 0.625rem 1rem;
		font-weight: 500;
	}

	.row .cell {
		padding-block: 0.75rem;
	}

	.row .question {
		display: none;
	}

	.cell.same .answer {
		color: var(--color-text-faint);
	}

	.cell.fixed {
		margin: 0;
		color: var(--color-text-muted);
	}

	.remove {
		align-self: start;
		margin: 0.625rem 0.25rem;
		border-color: transparent;
		border-left: var(--border-hairline);
		background: transparent;
	}

	.row > .remove {
		height: calc(100% - 1.25rem);
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
	}

	.adders .muted {
		margin-right: 1rem;
	}

	@media (max-width: 64rem) {
		.lines {
			--columns: 8rem repeat(4, minmax(0, 1fr)) 2.75rem;
		}
	}

	/* On a phone the points stack two by two, and a step is a short list. */
	@media (max-width: 48rem) {
		.line.main {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}

		.line.main .lead {
			grid-column: 1 / -1;
			padding-bottom: 0.5rem;
		}

		.line.main .point {
			border-top: var(--border-hairline);
		}

		.line.main .point:nth-of-type(odd) {
			border-left: 0;
		}

		.main .point + .point::before {
			display: none;
		}

		.end {
			display: none;
		}

		.line.row {
			grid-template-columns: minmax(0, 1fr) 2.75rem;
			padding-bottom: 0.25rem;
		}

		.row .cell {
			grid-column: 1 / -1;
			border-left: 0;
			padding: 0.5rem 1rem;
		}

		.row .question {
			display: block;
		}

		.row > .remove {
			grid-row: 1;
			grid-column: 2;
			height: auto;
			border-left: 0;
		}
	}
</style>
