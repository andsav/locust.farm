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
		type OnConnectEnd,
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
	import { pictureFor } from '../model/presets.ts';
	import { EVIDENCE_ORDER, EVIDENCE_WORDS, who } from '../model/words.ts';
	import { drawDiagram } from '../ui/diagrams.ts';
	import Icon from '../ui/Icon.svelte';
	import { tip } from '../ui/tooltip.ts';
	import FlowBridge, { type FlowBridge as Bridge } from './FlowBridge.svelte';
	import { NODE_HEIGHT, NODE_WIDTH, placeStages } from './layout.ts';
	import StageEdge, { type StageEdgeType } from './StageEdge.svelte';
	import StageNode, { type StageChip, type StageNodeType } from './StageNode.svelte';

	let {
		document,
		selected,
		panelOpen,
		panelWidth,
		problems,
		compact,
		frame,
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
		/** Changes when a different blueprint is loaded, so the map frames it afresh. */
		frame: number;
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

	function chips(name: string): StageChip[] {
		const stage = document.blueprint.flow[name];
		const out: StageChip[] = [
			{ icon: 'tray-arrow-down', text: who(stage.recipients), tip: 'Work goes to' },
			{
				icon: 'play-circle',
				text: stage.runner.kind === 'role' ? stage.runner.name : 'a specific person',
				tip: 'Run by'
			}
		];
		if (stage.task_type !== null) out.push({ icon: 'stack', text: 'own rule', tip: 'Done rule' });
		return out;
	}

	const picture = $derived(pictureFor(document.blueprint));

	function draw(svg: SVGSVGElement, id: string) {
		drawDiagram(svg, id);
		return { update: (next: string) => drawDiagram(svg, next) };
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
		requestAnimationFrame(() => bridge?.fitView({ padding: 0.35, maxZoom: 1.25, duration: 0 }));
	});
	$effect(() => {
		if (order.length === 0 || frame >= 0) framed = false;
	});

	// Frame them again when the map changes size, such as when the window is resized.
	$effect(() => {
		const element = container;
		if (!element) return;
		let width = element.clientWidth;
		let height = element.clientHeight;
		let frame = 0;
		const observer = new ResizeObserver(() => {
			if (element.clientWidth === width && element.clientHeight === height) return;
			width = element.clientWidth;
			height = element.clientHeight;
			cancelAnimationFrame(frame);
			frame = requestAnimationFrame(() => {
				if (order.length > 0) bridge?.fitView({ padding: 0.35, maxZoom: 1.25, duration: 0 });
			});
		});
		observer.observe(element);
		return () => {
			observer.disconnect();
			cancelAnimationFrame(frame);
		};
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

	/** The document with every stage pinned where it is now, so adding one moves no other. */
	function pinned(): EditorDocument {
		return { ...document, layout: { ...document.layout, stages: { ...positions } } };
	}

	// Dropping a connection on empty map adds a stage that waits for the first one, as in Catalyst.
	const connectEnd: OnConnectEnd = (event, state) => {
		if (!bridge || !state.fromNode || state.toNode || state.isValid) return;
		const point = 'changedTouches' in event ? event.changedTouches[0] : event;
		const at = bridge.screenToFlowPosition({ x: point.clientX, y: point.clientY });
		const from = state.fromNode.id;
		const name = freeStageName(document.blueprint);
		const added = addStage(pinned(), name, {
			x: Math.round(at.x),
			y: Math.round(at.y - NODE_HEIGHT / 2)
		});
		onchange(addRequirement(added, from, name, 'completion'));
		onselect(name);
		onannounce(`Added stage "${name}". It starts when "${from}" is complete.`);
	};

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
		onchange(addStage(position ? pinned() : document, name, position));
		onselect(name);
		onannounce(`Added stage "${name}". Its settings are open.`);
	}

	function tidy() {
		onchange({ ...structuredClone(document), layout: { ...document.layout, stages: {} } });
		requestAnimationFrame(() => bridge?.fitView({ padding: 0.35, maxZoom: 1.25, duration: 220 }));
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
	{#if order.length === 0}
		<svg
			class="diagram picture"
			style:--covered={`${panelOpen ? panelWidth : 0}px`}
			viewBox="0 0 320 180"
			aria-hidden="true"
			use:draw={picture}
		></svg>
	{/if}

	<div class="toolbar" role="toolbar" aria-label="Stage map controls">
		<button
			type="button"
			class="add-stage"
			aria-label="Add a stage"
			use:tip={{
				label: 'Add a stage',
				description:
					'Stages are steps that work moves through in order, such as draft, then review. Drag from a stage to connect it, or drop on empty space for a new stage.'
			}}
			onclick={addAStage}
		>
			<Icon name="plus" size={20} />
		</button>
		{#if compact}
			<span class="spacer"></span>
			<button
				type="button"
				aria-label={fullScreen ? 'Close the map' : 'Open the map full screen'}
				use:tip={fullScreen ? 'Close the map' : 'Full screen'}
				onclick={() => (fullScreen = !fullScreen)}
			>
				<Icon name={fullScreen ? 'corners-in' : 'corners-out'} />
			</button>
		{/if}
	</div>

	{#if order.length > 0 && interactive}
		<div class="zoom" role="toolbar" aria-label="Zoom">
			<button type="button" aria-label="Zoom in" use:tip={'Zoom in'} onclick={() => zoom(1.25)}>
				<Icon name="plus" size={16} />
			</button>
			<button
				type="button"
				aria-label="Zoom out"
				use:tip={'Zoom out'}
				onclick={() => zoom(1 / 1.25)}
			>
				<Icon name="minus" size={16} />
			</button>
			<button
				type="button"
				aria-label="Fit"
				use:tip={'Fit to the map'}
				onclick={() => bridge?.fitView({ padding: 0.35, maxZoom: 1.25, duration: 180 })}
			>
				<Icon name="frame-corners" size={16} />
			</button>
			{#if order.length > 1}
				<button type="button" aria-label="Tidy" use:tip={'Tidy'} onclick={tidy}>
					<Icon name="magic-wand" size={16} />
				</button>
			{/if}
		</div>
	{/if}

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
		connectionRadius={80}
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
		onconnectend={connectEnd}
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
		gap: 0.375rem;
		pointer-events: none;
	}

	.toolbar > *,
	.zoom > * {
		pointer-events: auto;
	}

	.spacer {
		flex: 1;
	}

	button {
		display: grid;
		place-items: center;
		width: 2.25rem;
		height: 2.25rem;
		padding: 0;
		border: var(--border-hairline);
		background: var(--color-surface);
		color: var(--color-text-muted);
		font: var(--text-ui);
		cursor: pointer;
	}

	button:hover {
		border-color: var(--color-text-faint);
		color: var(--color-text);
	}

	button:focus-visible {
		outline: 1px solid var(--color-accent);
		outline-offset: 1px;
	}

	.add-stage {
		width: 2.75rem;
		height: 2.75rem;
		box-shadow: 0 6px 20px rgb(0 0 0 / 0.55);
		border-color: var(--color-accent);
		border-radius: 50%;
		color: var(--color-accent);
	}

	.add-stage:hover {
		border-color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 16%, var(--color-surface));
		color: var(--color-text);
	}

	.zoom {
		position: absolute;
		bottom: 0.75rem;
		left: 0.75rem;
		z-index: 5;
		display: grid;
		grid-template-columns: repeat(2, 2.25rem);
		gap: 1px;
		border: var(--border-hairline);
		background: var(--color-border);
		box-shadow: 0 6px 20px rgb(0 0 0 / 0.55);
	}

	.zoom button {
		border: 0;
	}

	.picture {
		position: absolute;
		top: 50%;
		left: calc(50% - var(--covered) / 2);
		width: min(46rem, calc(86% - var(--covered)));
		height: auto;
		max-height: 80%;
		transform: translate(-50%, -50%);
		pointer-events: none;
		transition:
			left var(--duration-slow) cubic-bezier(0.16, 1, 0.3, 1),
			width var(--duration-slow) cubic-bezier(0.16, 1, 0.3, 1);
	}

	/* The picture is drawn at twice its size or more; keep its words small. */
	.picture :global(.d-label) {
		font-size: 5px;
	}

	.picture :global(.d-edge),
	.picture :global(.d-node),
	.picture :global(.d-stage),
	.picture :global(.d-arrow) {
		vector-effect: non-scaling-stroke;
	}

	.refusal {
		position: absolute;
		bottom: 0.75rem;
		left: 50%;
		transform: translateX(-50%);
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

	.choices button,
	.connect .quiet {
		display: block;
		width: auto;
		height: auto;
		min-height: 2.25rem;
		padding: 0 0.75rem;
		color: var(--color-text);
		text-align: left;
	}

	.connect .quiet {
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
