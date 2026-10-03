<script lang="ts">
	import { untrack } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import { createSwarm, type GoalMode } from '#lib/swarm/swarm.ts';

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

	const swarm: Attachment<HTMLCanvasElement> = (canvas) => {
		// Created once per canvas: later prop changes go through `update`, not a rebuild.
		const instance = untrack(() => createSwarm(canvas, { agents, goalMode, trail, catchUp }));
		if (!instance) return;
		$effect(() => instance.update({ agents, goalMode, trail, catchUp }));
		return instance.destroy;
	};
</script>

<!-- Decorative backdrop: fills its nearest positioned ancestor, behind that ancestor's content. -->
<canvas aria-hidden="true" {@attach swarm}></canvas>

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
