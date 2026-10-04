// Places stages that have no saved position, left to right in the order they
// wait for each other. Adapted from Polaris's workflow/layout.ts (dagre layout
// of step graphs) for stage nodes; positions already saved are kept.

import dagre from '@dagrejs/dagre';
import type { Point } from '../model/document.ts';

export const NODE_WIDTH = 216;
export const NODE_HEIGHT = 46;

export interface LayoutInput {
	names: string[];
	/** Pairs [earlier, later]. */
	links: [string, string][];
	positions: Record<string, Point>;
}

/** Positions for every stage; saved positions win. */
export function placeStages(input: LayoutInput): Record<string, Point> {
	const missing = input.names.filter((name) => !input.positions[name]);
	if (missing.length === 0) return { ...input.positions };
	const graph = new dagre.graphlib.Graph();
	graph.setGraph({ rankdir: 'LR', nodesep: 48, ranksep: 96, marginx: 0, marginy: 0 });
	graph.setDefaultEdgeLabel(() => ({}));
	for (const name of input.names) graph.setNode(name, { width: NODE_WIDTH, height: NODE_HEIGHT });
	for (const [from, to] of input.links) {
		if (input.names.includes(from) && input.names.includes(to)) graph.setEdge(from, to);
	}
	dagre.layout(graph);
	const out: Record<string, Point> = { ...input.positions };
	// Place new stages beside the ones already placed, so they do not land on top of them.
	const placed = Object.values(input.positions);
	const shiftX = placed.length > 0 ? Math.max(...placed.map((p) => p.x)) + NODE_WIDTH + 96 : 0;
	for (const name of missing) {
		const node = graph.node(name);
		const offset = placed.length > 0 ? shiftX : 0;
		out[name] = {
			x: Math.round(node.x - NODE_WIDTH / 2 + offset),
			y: Math.round(node.y - NODE_HEIGHT / 2)
		};
	}
	return out;
}
