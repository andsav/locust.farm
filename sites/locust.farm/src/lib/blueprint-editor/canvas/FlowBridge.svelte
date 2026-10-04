<!--
	Forked from Polaris (crates/polaris/frontend/src/lib/components/blueprint/FitViewBridge.svelte
	at dreamcolor10 01d8aa3c4). xyflow's hooks only work inside <SvelteFlow>; this
	component hands them to the map that hosts it.
-->
<script lang="ts" module>
	import type { FitViewOptions } from '@xyflow/svelte';

	export type FlowBridge = {
		fitView: (options?: FitViewOptions) => Promise<boolean>;
		setViewport: (
			viewport: { x: number; y: number; zoom: number },
			options?: { duration?: number }
		) => Promise<boolean>;
		screenToFlowPosition: (position: { x: number; y: number }) => { x: number; y: number };
		flowToScreenPosition: (position: { x: number; y: number }) => { x: number; y: number };
		getViewport: () => { x: number; y: number; zoom: number };
	};
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import { useSvelteFlow } from '@xyflow/svelte';

	let { onReady }: { onReady?: (bridge: FlowBridge) => void } = $props();

	const { fitView, setViewport, screenToFlowPosition, flowToScreenPosition, getViewport } =
		useSvelteFlow();

	$effect(() => {
		const ready = onReady;
		untrack(() =>
			ready?.({ fitView, setViewport, screenToFlowPosition, flowToScreenPosition, getViewport })
		);
	});
</script>
