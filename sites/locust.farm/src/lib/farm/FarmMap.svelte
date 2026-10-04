<script lang="ts">
	import { onMount, tick } from 'svelte';
	import type { FarmSnapshot } from './types.ts';
	import { associations, harnessName, mapState, stageColumns, statusName } from './model.ts';
	let {
		snapshot,
		compact = false,
		selected = null,
		onSelect = () => {}
	}: {
		snapshot: FarmSnapshot;
		compact?: boolean;
		selected?: number | null;
		onSelect?: (id: number) => void;
	} = $props();
	let body: HTMLDivElement;
	let edges = $state<{ id: string; path: string }[]>([]);
	async function measure() {
		await tick();
		if (!body) return;
		const bounds = body.getBoundingClientRect();
		edges = snapshot.stages.flatMap((stage) =>
			stage.prerequisites.flatMap((id) => {
				const from = body.querySelector(`[data-stage="${id}"] .capsule`)?.getBoundingClientRect();
				const to = body
					.querySelector(`[data-stage="${stage.id}"] .capsule`)
					?.getBoundingClientRect();
				if (!from || !to) return [];
				const vertical = to.top > from.bottom && Math.abs(to.left - from.left) < 30;
				const x1 = (vertical ? from.left + from.width / 2 : from.right) - bounds.left;
				const y1 = (vertical ? from.bottom : from.top + from.height / 2) - bounds.top;
				const x2 = (vertical ? to.left + to.width / 2 : to.left) - bounds.left;
				const y2 = (vertical ? to.top : to.top + to.height / 2) - bounds.top;
				const mid = vertical ? (y1 + y2) / 2 : (x1 + x2) / 2;
				return [
					{
						id: `${id}-${stage.id}`,
						path: vertical
							? `M ${from.right - bounds.left} ${from.top + from.height / 2 - bounds.top} C ${bounds.width + 12} ${from.top + from.height / 2 - bounds.top} ${bounds.width + 12} ${to.top + to.height / 2 - bounds.top} ${to.right - bounds.left} ${to.top + to.height / 2 - bounds.top}`
							: `M ${x1} ${y1} C ${mid} ${y1} ${mid} ${y2} ${x2} ${y2}`
					}
				];
			})
		);
	}
	$effect(() => {
		void snapshot;
		void measure();
	});
	onMount(() => {
		const observer = new ResizeObserver(() => {
			void measure();
		});
		observer.observe(body);
		return () => observer.disconnect();
	});
	const columns = $derived(stageColumns(snapshot));
	const unassociated = $derived(
		snapshot.agents.filter(
			(agent) =>
				!snapshot.attempts.some(
					(attempt) =>
						attempt.agent === agent.id &&
						snapshot.tasks.some((task) => task.id === attempt.task && task.round === attempt.round)
				)
		)
	);
</script>

{#snippet stageBlock(stageId: number)}
	{@const stage = snapshot.stages.find((stage) => stage.id === stageId)}
	{@const tasks = snapshot.tasks.filter((task) => (task.stage ?? 0) === stageId)}
	{@const done = tasks.length > 0 && tasks.every((task) => task.completed)}
	<div class="step" data-stage={stageId}>
		<div class="capsule">
			<span class="disc">{stage?.id ?? '·'}</span><span class="sname"
				>{stage?.label ?? 'Unstaged work'}</span
			>
		</div>
		{#if !compact}
			<p class="caption">
				{tasks.length}
				{tasks.length === 1 ? 'task' : 'tasks'}{#if done}
					· ✓ completed{/if}
			</p>
			{#if stage?.prerequisites.length}<p class="deps">
					After {stage.prerequisites
						.map((id) => snapshot.stages.find((item) => item.id === id)?.label ?? `stage ${id}`)
						.join(' + ')}
				</p>{/if}
		{/if}
		<div class="holders">
			{#each associations(snapshot, stage?.id ?? null) as { agent, attempts } (agent.id)}
				<span
					class="agent"
					title={`${agent.name} · ${harnessName(agent.harness)}: ${attempts.map(({ task, attempt }) => `${task.reference} ${attempt.state}`).join(', ')}`}
				>
					{#if !compact}<span class="num">{agent.id}</span>{agent.name} · {harnessName(
							agent.harness
						)}<span class="holds"
							>{attempts
								.map(({ task }) => task.reference)
								.filter((id, i, all) => all.indexOf(id) === i)
								.join(', ')}</span
						>{/if}
				</span>
			{/each}
		</div>
		<div class="marks">
			{#each tasks as task (task.id)}
				{#if compact}<span
						class="mark"
						data-state={mapState(task)}
						title={`${task.reference}: ${statusName(task.state)}`}><span class="dot"></span></span
					>
				{:else}<button
						class="mark"
						class:changed={selected === task.id}
						data-state={mapState(task)}
						aria-label={`Task ${task.reference}, ${statusName(task.state)}`}
						aria-pressed={selected === task.id}
						onclick={() => onSelect(task.id)}
						><span class="dot"></span><span class="ref">{task.reference}</span></button
					>{/if}
			{/each}
		</div>
		{#if !compact && !tasks.length}<p class="caption">No tasks</p>{/if}
	</div>
{/snippet}

<div class="map" aria-label={compact ? 'Farm stage preview' : 'Formation stages and reported work'}>
	<div class="map-body" bind:this={body}>
		<svg class="edges" aria-hidden="true"
			><defs
				><marker
					id={`arrow-${compact ? 'preview' : 'full'}-${snapshot.farm_id}`}
					viewBox="0 0 8 8"
					refX="7"
					refY="4"
					markerWidth="5"
					markerHeight="5"
					orient="auto-start-reverse"><path d="M 0 0 L 8 4 L 0 8" class="head-unmet" /></marker
				></defs
			>{#each edges as edge (edge.id)}<path
					class="edge"
					d={edge.path}
					marker-end={`url(#arrow-${compact ? 'preview' : 'full'}-${snapshot.farm_id})`}
				/>{/each}</svg
		>
		<div class="cols" style={`--cols: ${columns.length}`}>
			{#each columns as column, columnIndex (columnIndex)}
				<div class="col">
					{#each column as stageId (stageId)}
						{@render stageBlock(stageId)}
					{/each}
				</div>
			{/each}
		</div>
		{#if snapshot.stages.length && snapshot.tasks.some((task) => task.stage == null)}<div
				class="unstaged"
			>
				{@render stageBlock(0)}
			</div>{/if}
	</div>
	<div class="tray">
		{#if !compact}<span class="label">Agents without attempts</span>{/if}
		{#each unassociated as agent (agent.id)}<span
				class="agent free"
				title={`${agent.name} · ${harnessName(agent.harness)}`}
				>{#if !compact}<span class="num">{agent.id}</span>{agent.name} · {harnessName(
						agent.harness
					)}{/if}</span
			>{/each}
		{#if !compact && !unassociated.length}<span class="caption">None</span>{/if}
	</div>
	{#if compact}<div class="preview-progress">
			<i
				style={`width:${snapshot.tasks.length ? (snapshot.tasks.filter((task) => task.completed).length / snapshot.tasks.length) * 100 : 0}%`}
			></i>
		</div>{/if}
</div>
