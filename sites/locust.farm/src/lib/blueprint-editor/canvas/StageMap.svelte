<!--
	The stage map: stages as nodes, and "starts when" prerequisites as arrows.
	Nothing else is drawn. Positions are layout only and never change the
	blueprint.
-->
<script lang="ts">
	import '@xyflow/svelte/dist/style.css';
	import {
		Background,
		BackgroundVariant,
		MarkerType,
		SvelteFlow,
		type Connection,
		type NodeTypes,
		type EdgeTypes
	} from '@xyflow/svelte';
	import type { EvidenceKind } from '../contract/types.ts';
	import type { EditorDocument, Point } from '../model/document.ts';
	import {
		addRequirement,
		addStage,
		freeStageName,
		moveStage,
		removeStage,
		stageOrder,
		wouldLoop
	} from '../model/edit.ts';
	import { EVIDENCE_ORDER, EVIDENCE_WORDS, who } from '../model/words.ts';
	import FlowBridge, { type FlowBridge as Bridge } from './FlowBridge.svelte';
	import { NODE_HEIGHT, NODE_WIDTH, placeStages } from './layout.ts';
	import StageEdge, { type StageEdgeType } from './StageEdge.svelte';
	import StageNode, { type StageNodeType } from './StageNode.svelte';

	let {
		document,
		selected,
		panelOpen,
		panelWidth,
		problems,
		compact,
		onchange,
		onselect,
		onannounce
	}: {
		document: EditorDocument;
		selected: string | null;
		panelOpen: boolean;
		/** Width in pixels the side panel covers on the right of the map. */
		panelWidth: number;
		problems: Record<string, number>;
		/** Phones: the map is a preview until opened full screen. */
		compact: boolean;
		onchange: (next: EditorDocument) => void;
		onselect: (name: string | null) => void;
		onannounce: (message: string) => void;
	} = $props();

	const nodeTypes: NodeTypes = { stage: StageNode };
	const edgeTypes: EdgeTypes = { stage: StageEdge };

	let container: HTMLDivElement | undefined = $state();
	let bridge: Bridge | null = $state(null);
	let fullScreen = $state(false);
	const interactive = $derived(!compact || fullScreen);

	const order = $derived(stageOrder(document));
	const positions = $derived(
		placeStages({
			names: order,
			links: order.flatMap((name) =>
				document.blueprint.flow[name].requires.map((item): [string, string] => [item.stage, name])
			),
			positions: document.layout.stages
		})
	);

	function chips(name: string): string[] {
		const stage = document.blueprint.flow[name];
		const out = [`work goes to ${who(stage.recipients)}`];
		out.push(
			stage.runner.kind === 'role'
				? `run by the "${stage.runner.name}" role`
				: 'run by a specific person'
		);
		if (stage.task_type !== null) out.push(`own done rule`);
		return out;
	}

	function startsText(name: string): string {
		const requires = document.blueprint.flow[name].requires;
		return requires.length === 0
			? 'Can start any time'
			: `Starts when ${requires.map((item) => EVIDENCE_WORDS[item.evidence].when(item.stage)).join(' and ')}`;
	}

	let nodes = $state.raw<StageNodeType[]>([]);
	let edges = $state.raw<StageEdgeType[]>([]);

	$effect(() => {
		nodes = order.map((name, index) => ({
			id: name,
			type: 'stage',
			position: positions[name],
			selected: name === selected,
			width: NODE_WIDTH,
			height: NODE_HEIGHT,
			ariaLabel: `Stage ${index + 1}: ${name}. ${startsText(name)}.${
				problems[name] ? ` ${problems[name]} problem${problems[name] === 1 ? '' : 's'}.` : ''
			}`,
			data: {
				name,
				number: index + 1,
				chips: chips(name),
				problems: problems[name] ?? 0,
				panelOpen: panelOpen && name === selected
			}
		}));
		edges = order.flatMap((name) =>
			document.blueprint.flow[name].requires
				.map((item, index) => ({ item, index }))
				.filter(({ item }) => Object.hasOwn(document.blueprint.flow, item.stage))
				.map(({ item, index }) => ({
					id: `${item.stage}→${name}#${index}`,
					type: 'stage' as const,
					source: item.stage,
					target: name,
					markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
					data: { label: EVIDENCE_WORDS[item.evidence].arrow, requirement: index }
				}))
		);
	});

	// Frame the stages when the map first has some.
	let framed = $state(false);
	$effect(() => {
		if (!bridge || order.length === 0 || framed) return;
		framed = true;
		requestAnimationFrame(() => bridge?.fitView({ padding: 0.35, maxZoom: 1.1, duration: 0 }));
	});
	$effect(() => {
		if (order.length === 0) framed = false;
	});

	// Keep the selected stage visible beside the side panel, as Polaris's panel camera does.
	$effect(() => {
		const name = selected;
		const open = panelOpen;
		if (!bridge || !container || !name || !open || !positions[name]) return;
		const rect = container.getBoundingClientRect();
		const point = positions[name];
		const right = bridge.flowToScreenPosition({ x: point.x + NODE_WIDTH, y: point.y });
		const left = bridge.flowToScreenPosition({ x: point.x, y: point.y });
		const visibleRight = rect.right - panelWidth - 24;
		const viewport = bridge.getViewport();
		let shift = 0;
		if (right.x > visibleRight) shift = visibleRight - right.x;
		if (left.x + shift < rect.left + 24) shift = rect.left + 24 - left.x;
		if (shift !== 0) bridge.setViewport({ ...viewport, x: viewport.x + shift }, { duration: 220 });
	});

	// Connecting two stages asks what the later stage waits for.
	let pending = $state<{ from: string; to: string | null } | null>(null);
	let refusal = $state<string | null>(null);

	function connect(connection: Connection) {
		const from = connection.source;
		const to = connection.target;
		if (wouldLoop(document.blueprint, from, to)) {
			refusal = `That would make "${from}" wait for itself.`;
			onannounce(refusal);
			return;
		}
		refusal = null;
		pending = { from, to };
	}

	function choose(evidence: EvidenceKind, to: string | null) {
		if (!pending || !to) return;
		const { from } = pending;
		if (wouldLoop(document.blueprint, from, to)) {
			refusal = `That would make "${from}" wait for itself.`;
			onannounce(refusal);
			pending = null;
			return;
		}
		onchange(addRequirement(document, from, to, evidence));
		onannounce(`"${to}" now starts when ${EVIDENCE_WORDS[evidence].when(from)}.`);
		pending = null;
	}

	let keyboardTarget = $state<string>('');

	function onkeydown(event: KeyboardEvent) {
		const element = event.target as HTMLElement;
		const node = element.closest?.('.svelte-flow__node') as HTMLElement | null;
		const name = node?.dataset.id;
		if (!name || !Object.hasOwn(document.blueprint.flow, name)) return;
		if (event.key === 'Enter') {
			event.preventDefault();
			onselect(name);
		} else if (event.key === 'Delete' || event.key === 'Backspace') {
			event.preventDefault();
			onchange(removeStage(document, name));
			onannounce(`Removed stage "${name}". Undo brings it back.`);
		} else if (event.key === 'c' || event.key === 'C') {
			event.preventDefault();
			keyboardTarget = order.find((other) => other !== name) ?? '';
			pending = { from: name, to: null };
		}
	}

	// Keys pressed on a focused stage: listened for on the map, which contains the stages.
	$effect(() => {
		const element = container;
		if (!element) return;
		element.addEventListener('keydown', onkeydown);
		return () => element.removeEventListener('keydown', onkeydown);
	});

	function addAStage() {
		const name = freeStageName(document.blueprint);
		let position: Point | null = null;
		if (bridge && container) {
			const rect = container.getBoundingClientRect();
			const centre = bridge.screenToFlowPosition({
				x: rect.left + (rect.width - (panelOpen ? panelWidth : 0)) / 2,
				y: rect.top + rect.height / 2
			});
			position = {
				x: Math.round(centre.x - NODE_WIDTH / 2),
				y: Math.round(centre.y - NODE_HEIGHT / 2)
			};
			const taken = Object.values(positions).some(
				(p) => Math.abs(p.x - position!.x) < NODE_WIDTH && Math.abs(p.y - position!.y) < 80
			);
			if (taken) position = null;
		}
		onchange(addStage(document, name, position));
		onselect(name);
		onannounce(`Added stage "${name}". Its settings are open.`);
	}

	function tidy() {
		onchange({ ...structuredClone(document), layout: { ...document.layout, stages: {} } });
		requestAnimationFrame(() => bridge?.fitView({ padding: 0.35, maxZoom: 1.1, duration: 220 }));
	}

	function zoom(factor: number) {
		if (!bridge) return;
		const viewport = bridge.getViewport();
		const nextZoom = Math.min(2, Math.max(0.5, viewport.zoom * factor));
		if (!container) return;
		const rect = container.getBoundingClientRect();
		const cx = rect.width / 2;
		const cy = rect.height / 2;
		bridge.setViewport(
			{
				zoom: nextZoom,
				x: cx - ((cx - viewport.x) * nextZoom) / viewport.zoom,
				y: cy - ((cy - viewport.y) * nextZoom) / viewport.zoom
			},
			{ duration: 140 }
		);
	}
</script>

<div
	class="map"
	class:full-screen={fullScreen}
	class:preview={!interactive}
	bind:this={container}
	role="group"
	aria-label="Stage map"
>
	<div class="toolbar" role="toolbar" aria-label="Stage map controls">
		{#if order.length > 0}
			<button type="button" onclick={addAStage}>Add a stage</button>
		{/if}
		{#if order.length > 1}
			<button type="button" onclick={tidy}>Tidy</button>
		{/if}
		<span class="spacer"></span>
		<button type="button" onclick={() => zoom(1 / 1.25)}>Zoom out</button>
		<button type="button" onclick={() => zoom(1.25)}>Zoom in</button>
		<button
			type="button"
			onclick={() => bridge?.fitView({ padding: 0.35, maxZoom: 1.1, duration: 180 })}
		>
			Fit
		</button>
		{#if compact}
			<button type="button" onclick={() => (fullScreen = !fullScreen)}>
				{fullScreen ? 'Close the map' : 'Open the map full screen'}
			</button>
		{/if}
	</div>

	<p class="help visually-hidden" id="stage-map-help">
		Tab moves between stages. Enter opens a stage's settings. C connects it to another stage. Delete
		removes it; undo brings it back. Every stage is also listed below the map.
	</p>

	<SvelteFlow
		bind:nodes
		bind:edges
		{nodeTypes}
		{edgeTypes}
		colorMode="dark"
		deleteKey={null}
		selectionKey={null}
		multiSelectionKey={null}
		minZoom={0.5}
		maxZoom={2}
		nodesDraggable={interactive}
		nodesConnectable={interactive}
		panOnDrag={interactive}
		zoomOnScroll={false}
		panOnScroll={interactive}
		zoomOnPinch={interactive}
		preventScrolling={interactive}
		edgesFocusable={false}
		connectionRadius={40}
		proOptions={{ hideAttribution: true }}
		ariaLabelConfig={{
			'node.a11yDescription.default':
				"Press Enter to open the stage's settings, C to connect it to another stage, Delete to remove it.",
			'node.a11yDescription.keyboardDisabled':
				"Press Enter to open the stage's settings, C to connect it to another stage, Delete to remove it.",
			'node.a11yDescription.ariaLiveMessage': () => '',
			'edge.a11yDescription.default':
				"An arrow between stages. Change it in the later stage's settings.",
			'handle.ariaLabel': 'Connection point. With a keyboard, focus the stage and press C.'
		}}
		onnodeclick={({ node }) => onselect(node.id)}
		onedgeclick={({ edge }) => onselect(edge.target)}
		onpaneclick={() => onselect(null)}
		onconnect={connect}
		onnodedragstop={({ nodes: moved }) => {
			let next = document;
			for (const node of moved) {
				const before = positions[node.id];
				if (!before || before.x !== node.position.x || before.y !== node.position.y) {
					next = moveStage(next, node.id, {
						x: Math.round(node.position.x),
						y: Math.round(node.position.y)
					});
				}
			}
			if (next !== document) onchange(next);
		}}
	>
		<FlowBridge onReady={(value) => (bridge = value)} />
		<Background
			variant={BackgroundVariant.Dots}
			gap={22}
			size={1.2}
			patternColor="var(--color-grid)"
		/>
	</SvelteFlow>

	{#if order.length === 0}
		<div class="empty">
			<p>
				This blueprint has no stages. Work isn't done in a fixed order. Add a stage if work should
				move from one step to the next.
			</p>
			<button type="button" onclick={addAStage}>Add a stage</button>
		</div>
	{/if}

	{#if refusal}
		<p class="refusal" role="status">{refusal}</p>
	{/if}

	{#if pending}
		<div class="connect" role="dialog" aria-label="Connect stages">
			{#if pending.to === null}
				<label>
					<span>Which stage should wait for "{pending.from}"?</span>
					<select bind:value={keyboardTarget}>
						{#each order.filter((name) => name !== pending?.from) as name (name)}
							<option value={name}>{name}</option>
						{/each}
					</select>
				</label>
			{/if}
			<p>
				Start "{pending.to ?? keyboardTarget}" when "{pending.from}"…
			</p>
			<div class="choices">
				{#each EVIDENCE_ORDER as evidence (evidence)}
					<button type="button" onclick={() => choose(evidence, pending?.to ?? keyboardTarget)}>
						{EVIDENCE_WORDS[evidence].short}
					</button>
				{/each}
			</div>
			<button type="button" class="quiet" onclick={() => (pending = null)}>Cancel</button>
		</div>
	{/if}
</div>

<style>
	.map {
		position: relative;
		height: 100%;
		min-height: 22rem;
		border: var(--border-hairline);
		background: var(--color-bg);
		font: var(--text-ui);
	}

	.map.full-screen {
		position: fixed;
		inset: 0;
		z-index: 30;
		min-height: 0;
	}

	.toolbar {
		position: absolute;
		top: 0.75rem;
		right: 0.75rem;
		left: 0.75rem;
		z-index: 5;
		display: flex;
		flex-wrap: wrap;
		gap: 0.375rem;
		pointer-events: none;
	}

	.toolbar > * {
		pointer-events: auto;
	}

	.spacer {
		flex: 1;
	}

	button {
		min-height: 2.25rem;
		padding: 0 0.75rem;
		border: var(--border-hairline);
		background: var(--color-surface);
		color: var(--color-text);
		font: var(--text-ui);
		cursor: pointer;
	}

	button:hover {
		border-color: var(--color-text-faint);
	}

	.empty {
		position: absolute;
		inset: 0;
		display: grid;
		place-content: center;
		justify-items: center;
		gap: 1rem;
		padding: 3.5rem 1.5rem 1.5rem;
		text-align: center;
		pointer-events: none;
	}

	.empty p {
		max-width: 28rem;
		color: var(--color-text-muted);
	}

	.empty button {
		pointer-events: auto;
		border-color: var(--color-accent);
	}

	.refusal {
		position: absolute;
		bottom: 0.75rem;
		left: 0.75rem;
		z-index: 5;
		padding: 0.375rem 0.75rem;
		border: 1px solid var(--color-accent);
		background: var(--color-bg);
	}

	.connect {
		position: absolute;
		top: 50%;
		left: 50%;
		z-index: 10;
		display: grid;
		gap: 0.75rem;
		width: min(22rem, calc(100% - 2rem));
		padding: 1rem;
		border: 1px solid var(--color-accent);
		background: var(--color-surface);
		transform: translate(-50%, -50%);
	}

	.connect label {
		display: grid;
		gap: 0.375rem;
	}

	.connect select {
		min-height: 2.25rem;
		border: var(--border-hairline);
		background: var(--color-bg);
		color: var(--color-text);
		font: var(--text-ui);
	}

	.choices {
		display: grid;
		gap: 0.375rem;
	}

	.choices button {
		text-align: left;
	}

	.quiet {
		justify-self: start;
		border-color: transparent;
		background: transparent;
		color: var(--color-text-muted);
	}

	.map :global(.svelte-flow) {
		background: transparent;
		--xy-background-color: transparent;
	}

	.map.preview :global(.svelte-flow__pane) {
		cursor: default;
	}

	.map :global(.stage-edge) {
		stroke: var(--color-text-faint);
		stroke-width: 1.5;
	}

	.map :global(.stage-edge.selected) {
		stroke: var(--color-accent);
	}

	.map :global(.svelte-flow__arrowhead polyline) {
		fill: var(--color-text-faint);
		stroke: var(--color-text-faint);
	}

	.map :global(.stage-edge-label) {
		padding: 1px 6px;
		border: var(--border-hairline);
		background: var(--color-bg);
		color: var(--color-text-muted);
		font: 400 10px / 1.4 var(--font-mono);
	}

	.map :global(.svelte-flow__edge-interaction) {
		stroke-width: 20;
	}

	.map :global(.svelte-flow__connectionline path) {
		stroke: var(--color-accent);
	}
</style>
