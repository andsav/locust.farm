<!--
	An arrow from an earlier stage to a later one, labelled in plain words
	("when draft is complete"). Each arrow is one entry in the later stage's
	"requires" list.
-->
<script lang="ts" module>
	import type { Edge } from '@xyflow/svelte';

	export type StageEdgeData = { label: string; requirement: number };
	export type StageEdgeType = Edge<StageEdgeData, 'stage'>;
</script>

<script lang="ts">
	import { BaseEdge, EdgeLabel, getBezierPath, type EdgeProps } from '@xyflow/svelte';

	let {
		id,
		sourceX,
		sourceY,
		targetX,
		targetY,
		sourcePosition,
		targetPosition,
		data,
		selected,
		markerEnd
	}: EdgeProps<StageEdgeType> = $props();

	const path = $derived(
		getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition })
	);
</script>

<BaseEdge
	{id}
	path={path[0]}
	{markerEnd}
	class={selected ? 'stage-edge selected' : 'stage-edge'}
	interactionWidth={20}
/>
<EdgeLabel x={path[1]} y={path[2]} class="stage-edge-label {selected ? 'selected' : ''}">
	{data?.label ?? ''}
</EdgeLabel>
