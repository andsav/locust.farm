<!--
	A stage on the map. Adapted from Polaris's capsule node
	(crates/polaris/frontend/src/lib/components/blueprint/nodes/NodeCapsule.svelte at
	dreamcolor10 01d8aa3c4): the same 216×46 pill with a round accent disc, rims that
	light on hover and selection, ports on the rim and an overlay that grows when the
	stage is selected, in the site's colours and font. Run states, inline renaming and
	tooltips are not carried over.
-->
<script lang="ts" module>
	import type { Node } from '@xyflow/svelte';

	export type StageNodeData = {
		name: string;
		/** Position in the order of stages, shown in the disc. */
		number: number;
		chips: string[];
		problems: number;
		panelOpen: boolean;
	};
	export type StageNodeType = Node<StageNodeData, 'stage'>;
</script>

<script lang="ts">
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';

	let { data, selected }: NodeProps<StageNodeType> = $props();
</script>

<div
	class="capsule"
	class:selected
	class:panel-open={data.panelOpen}
	class:error={data.problems > 0}
>
	<Handle type="target" position={Position.Left} isConnectableStart={false} />
	<div class="body">
		<div class="header">
			<span class="disc" aria-hidden="true">{data.number}</span>
			<span class="name">{data.name}</span>
			{#if data.problems > 0}
				<span class="problems"
					>{data.problems === 1 ? '1 problem' : `${data.problems} problems`}</span
				>
			{/if}
		</div>
		<div class="expansion">
			<div class="chips">
				{#each data.chips as chip (chip)}
					<span class="chip">{chip}</span>
				{/each}
			</div>
		</div>
	</div>
	<Handle type="source" position={Position.Right} />
</div>

<style>
	.capsule {
		position: relative;
		width: 216px;
		height: 46px;
		cursor: default;
		--accent: var(--color-accent);
	}

	.body {
		position: absolute;
		inset: 0;
		overflow: hidden;
		border: 1px solid var(--color-border);
		border-radius: 23px;
		background: var(--color-surface);
		color: var(--color-text);
		box-shadow: 0 6px 18px -10px rgb(0 0 0 / 0.8);
		transition:
			border-color var(--duration-fast),
			box-shadow var(--duration-fast),
			transform var(--duration-fast);
	}

	.capsule.selected .body {
		inset: -36px 0 auto;
		min-height: 118px;
		border-radius: 20px;
		border-color: color-mix(in srgb, var(--accent) 65%, transparent);
		box-shadow: 0 18px 40px -18px color-mix(in srgb, var(--accent) 55%, transparent);
	}

	.capsule:hover .body {
		border-color: color-mix(in srgb, var(--accent) 40%, transparent);
		transform: translateY(-1px);
		transition-duration: 0s;
	}

	.capsule.selected:hover .body {
		transform: none;
	}

	.capsule.error .body {
		border-color: color-mix(in srgb, var(--accent) 75%, transparent);
		border-style: dashed;
	}

	.capsule.panel-open .body {
		border-color: var(--accent);
		box-shadow:
			0 0 0 0.5px var(--accent),
			0 0 0 4.5px color-mix(in srgb, var(--accent) 22%, transparent),
			0 18px 40px -20px color-mix(in srgb, var(--accent) 50%, transparent);
	}

	:global(.svelte-flow__node:focus-visible) .capsule .body {
		outline: 1px solid var(--color-accent);
		outline-offset: 4px;
	}

	.header {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 44px;
		padding: 0 8px;
	}

	.capsule.selected .header {
		margin-top: 36px;
	}

	.disc {
		display: grid;
		flex: none;
		place-items: center;
		width: 30px;
		height: 30px;
		border-radius: 50%;
		background: color-mix(in srgb, var(--accent) 18%, var(--color-surface));
		color: var(--accent);
		font: 500 12px / 1 var(--font-mono);
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		font: 500 13px / 1.25 var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.problems {
		flex: none;
		padding: 2px 6px;
		border: 1px solid color-mix(in srgb, var(--accent) 60%, transparent);
		border-radius: 999px;
		color: var(--color-text);
		font: 400 10px / 1.2 var(--font-mono);
		white-space: nowrap;
	}

	.expansion {
		display: none;
		padding: 2px 12px 12px;
	}

	.capsule.selected .expansion {
		display: block;
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.chip {
		max-width: 100%;
		overflow: hidden;
		padding: 2px 8px;
		border: 1px solid var(--color-border);
		border-radius: 999px;
		color: var(--color-text-muted);
		font: 400 10px / 1.4 var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* Ports sit on the rim and show on hover, on selection and while connecting. */
	.capsule :global(.svelte-flow__handle) {
		z-index: 1;
		width: 10px;
		height: 10px;
		min-width: 0;
		min-height: 0;
		border: 0;
		border-radius: 0;
		background: transparent;
	}

	.capsule :global(.svelte-flow__handle-left) {
		top: 50%;
		left: 0;
		transform: translateY(-50%);
	}

	.capsule :global(.svelte-flow__handle-right) {
		top: 50%;
		right: 0;
		left: auto;
		transform: translateY(-50%);
	}

	.capsule :global(.svelte-flow__handle)::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		width: 10px;
		height: 10px;
		border: 2.5px solid var(--accent);
		border-radius: 50%;
		background: var(--color-surface);
		opacity: 0;
		transform: scale(0.4);
		pointer-events: none;
		transition:
			opacity var(--duration-fast),
			transform var(--duration-fast);
	}

	.capsule :global(.svelte-flow__handle-left)::before {
		left: -5px;
	}

	.capsule :global(.svelte-flow__handle-right)::before {
		left: 5px;
	}

	.capsule :global(.svelte-flow__handle)::after {
		content: '';
		position: absolute;
		inset: -10px;
	}

	.capsule:hover :global(.svelte-flow__handle)::before,
	.capsule.selected :global(.svelte-flow__handle)::before,
	:global(.connecting) .capsule :global(.svelte-flow__handle)::before {
		opacity: 1;
		transform: none;
	}

	@media (pointer: coarse) {
		.capsule :global(.svelte-flow__handle)::after {
			inset: -18px;
		}
	}
</style>
