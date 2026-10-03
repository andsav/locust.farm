<script lang="ts">
	import { swarm, type GoalMode } from '#lib/swarm/swarm.ts';

	interface Props {
		/** Number of agents in the swarm. */
		agents?: number;
		/** What the swarm chases: a wandering goal, the cursor, or the last click. */
		goalMode?: GoalMode;
		/** How much of each trail fades per step, from 0 (never fades) to 1 (no trail). */
		trail?: number;
		/** Extra pull toward the cursor per 100 pixels of distance. 0 turns it off. */
		catchUp?: number;
	}

	let { agents = 1400, goalMode = 'wander', trail = 0.28, catchUp = 0.3 }: Props = $props();
</script>

<!-- Decorative backdrop: fills its nearest positioned ancestor, behind that ancestor's content. -->
<canvas aria-hidden="true" {@attach swarm(() => ({ agents, goalMode, trail, catchUp }))}></canvas>

<style>
	canvas {
		position: absolute;
		inset: 0;
		z-index: -1;
		display: block;
		width: 100%;
		height: 100%;
	}
</style>
