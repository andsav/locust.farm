<!--
	An arrow from an earlier stage to a later one: one entry in the later stage's
	"requires" list. Arrows carry no words. A solid arrow waits until the earlier
	stage is complete; a dashed one starts sooner, on a published result, a review
	or a picked result. Hovering an arrow says which, as a tooltip.
-->
<script lang="ts" module>
	import type { Edge } from '@xyflow/svelte';

	export type StageEdgeData = {
		/** "review" starts when "draft" is complete. */
		sentence: string;
		/** Waits for something other than completion. */
		early: boolean;
		requirement: number;
	};
	export type StageEdgeType = Edge<StageEdgeData, 'stage'>;
</script>

<script lang="ts">
	import { BaseEdge, getBezierPath, type EdgeProps } from '@xyflow/svelte';
	import { tip } from '../ui/tooltip.ts';

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
	class={['stage-edge', selected && 'selected', data?.early && 'early'].filter(Boolean).join(' ')}
	interactionWidth={0}
/>
<path class="stage-edge-hit" d={path[0]} use:tip={data?.sentence ?? ''} />
