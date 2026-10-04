// Small animated pictures for the six ways of working. Grey dots are work and
// findings moving between people; orange marks a decision. Colours come from
// CSS classes styled with the site's tokens. With reduced motion each picture
// holds one frame that still tells its story.

const NS = 'http://www.w3.org/2000/svg';

interface P {
	x: number;
	y: number;
}

const P = (x: number, y: number): P => ({ x, y });

type Svg = SVGSVGElement;

function el(parent: Element, name: string, attrs: Record<string, string | number>): SVGElement {
	const node = document.createElementNS(NS, name);
	for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, String(value));
	parent.append(node);
	return node;
}

function bent(a: P, b: P, bend = 0.12): string {
	const mx = (a.x + b.x) / 2;
	const my = (a.y + b.y) / 2;
	const dx = b.x - a.x;
	const dy = b.y - a.y;
	return `M${a.x} ${a.y} Q${mx - dy * bend} ${my + dx * bend} ${b.x} ${b.y}`;
}

function times(values: number[]): string {
	return values.map((v) => Math.min(1, Math.max(0, v)).toFixed(4)).join(';');
}

function dot(
	svg: Svg,
	cycle: number,
	path: string,
	t0: number,
	t1: number,
	tone = 'work',
	r = 2.6
) {
	const c = el(svg, 'circle', { r, class: `d-dot d-${tone}`, opacity: 0 });
	el(c, 'animateMotion', {
		path,
		dur: `${cycle}s`,
		repeatCount: 'indefinite',
		calcMode: 'linear',
		keyPoints: '0;0;1;1',
		keyTimes: times([0, t0, t1, 1])
	});
	el(c, 'animate', {
		attributeName: 'opacity',
		dur: `${cycle}s`,
		repeatCount: 'indefinite',
		values: '0;0;1;1;0;0',
		keyTimes: times([0, t0, t0 + 0.005, t1 - 0.005, t1, 1])
	});
}

function pulse(svg: Svg, cycle: number, at: P, t: number, r0 = 8, r1 = 18, length = 0.12) {
	const ring = el(svg, 'circle', {
		cx: at.x,
		cy: at.y,
		r: r0,
		class: 'd-ring d-decision',
		opacity: 0
	});
	el(ring, 'animate', {
		attributeName: 'r',
		dur: `${cycle}s`,
		repeatCount: 'indefinite',
		values: `${r0};${r0};${r1};${r1}`,
		keyTimes: times([0, t, t + length, 1])
	});
	el(ring, 'animate', {
		attributeName: 'opacity',
		dur: `${cycle}s`,
		repeatCount: 'indefinite',
		values: '0;0;0.9;0;0',
		keyTimes: times([0, t, t + 0.004, t + length, 1])
	});
}

function glow(svg: Svg, cycle: number, at: P, r: number, t0: number, t1: number, tone = 'busy') {
	const ring = el(svg, 'circle', { cx: at.x, cy: at.y, r, class: `d-ring d-${tone}`, opacity: 0 });
	el(ring, 'animate', {
		attributeName: 'opacity',
		dur: `${cycle}s`,
		repeatCount: 'indefinite',
		values: '0;0;1;1;0;0',
		keyTimes: times([0, t0, t0 + 0.02, t1 - 0.02, t1, 1])
	});
}

function node(svg: Svg, at: P, r = 7, lead = false) {
	el(svg, 'circle', { cx: at.x, cy: at.y, r, class: lead ? 'd-node d-lead' : 'd-node' });
}

function edge(svg: Svg, a: P, b: P, bend = 0) {
	el(svg, 'path', { d: bent(a, b, bend), class: 'd-edge' });
}

const DRAWINGS: Record<
	string,
	{ cycle: number; still: number; draw: (svg: Svg, cycle: number) => void }
> = {
	open: {
		cycle: 9,
		still: 0.12,
		draw(svg, cycle) {
			const n = [P(70, 52), P(160, 30), P(252, 56), P(258, 132), P(160, 152), P(64, 126)];
			n.forEach((a, i) => n.slice(i + 1).forEach((b) => edge(svg, a, b)));
			n.forEach((a) => node(svg, a));
			const sends: [number, number, number, number][] = [
				[0, 2, 0.02, 0.16],
				[4, 1, 0.08, 0.2],
				[3, 5, 0.18, 0.33],
				[1, 3, 0.28, 0.4],
				[5, 2, 0.4, 0.55],
				[2, 4, 0.5, 0.62],
				[0, 3, 0.6, 0.75],
				[3, 1, 0.7, 0.82],
				[4, 0, 0.8, 0.94]
			];
			for (const [a, b, t0, t1] of sends) dot(svg, cycle, bent(n[a], n[b], 0.08), t0, t1);
			pulse(svg, cycle, n[2], 0.24);
			pulse(svg, cycle, n[5], 0.6);
			pulse(svg, cycle, n[1], 0.88);
		}
	},
	coordinator: {
		cycle: 8,
		still: 0.46,
		draw(svg, cycle) {
			const c = P(160, 88);
			const w = [-90, -18, 54, 126, 198].map((deg) => {
				const a = (deg * Math.PI) / 180;
				return P(160 + Math.cos(a) * 92, 88 + Math.sin(a) * 62);
			});
			w.forEach((p) => edge(svg, c, p));
			w.forEach((p) => node(svg, p));
			node(svg, c, 11, true);
			w.forEach((p, i) => {
				const out = 0.03 + i * 0.07;
				const back = out + 0.32;
				dot(svg, cycle, bent(c, p, 0.1), out, out + 0.1);
				glow(svg, cycle, p, 10, out + 0.1, back);
				dot(svg, cycle, bent(p, c, 0.1), back, back + 0.1, 'result');
				pulse(svg, cycle, c, back + 0.1, 11, 22);
			});
		}
	},
	'peer-review': {
		cycle: 9,
		still: 0.36,
		draw(svg, cycle) {
			const n = [P(90, 50), P(230, 50), P(230, 132), P(90, 132)];
			const links: [number, number][] = [
				[0, 1],
				[1, 2],
				[2, 3],
				[3, 0],
				[0, 2],
				[1, 3]
			];
			for (const [a, b] of links) edge(svg, n[a], n[b]);
			n.forEach((a) => node(svg, a));
			const reviews: [number, number, number][] = [
				[0, 1, 0.03],
				[2, 3, 0.28],
				[3, 0, 0.55]
			];
			for (const [author, reviewer, t] of reviews) {
				dot(svg, cycle, bent(n[author], n[reviewer], 0.12), t, t + 0.12, 'result');
				glow(svg, cycle, n[reviewer], 10, t + 0.12, t + 0.22);
				dot(svg, cycle, bent(n[reviewer], n[author], 0.12), t + 0.22, t + 0.34, 'decision', 2.4);
				pulse(svg, cycle, n[author], t + 0.34);
			}
		}
	},
	'review-panel': {
		cycle: 9,
		still: 0.66,
		draw(svg, cycle) {
			const a = P(72, 90);
			const r = [P(244, 52), P(244, 128)];
			r.forEach((p) => edge(svg, a, p, 0.06));
			r.forEach((p) => node(svg, p));
			node(svg, a, 9);
			r.forEach((p) => dot(svg, cycle, bent(a, p, 0.06), 0.04, 0.2, 'result'));
			r.forEach((p, i) => glow(svg, cycle, p, 10, 0.2, [0.36, 0.5][i]));
			dot(svg, cycle, bent(r[0], a, 0.06), 0.36, 0.5, 'decision', 2.4);
			dot(svg, cycle, bent(r[1], a, 0.06), 0.5, 0.64, 'decision', 2.4);
			pulse(svg, cycle, a, 0.64, 9, 20);
		}
	},
	'independent-attempts': {
		cycle: 9,
		still: 0.7,
		draw(svg, cycle) {
			const task = P(160, 26);
			const w = [P(78, 90), P(160, 90), P(242, 90)];
			const judge = P(160, 156);
			w.forEach((p) => {
				edge(svg, task, p);
				edge(svg, p, judge);
			});
			node(svg, task, 8);
			w.forEach((p) => node(svg, p));
			node(svg, judge, 9, true);
			w.forEach((p) => dot(svg, cycle, bent(task, p, 0), 0.03, 0.14));
			const attempts: [number, number][] = [
				[0, 0.3],
				[1, 0.22],
				[2, 0.4]
			];
			for (const [i, end] of attempts) {
				glow(svg, cycle, w[i], 10, 0.14, end);
				dot(svg, cycle, bent(w[i], judge, 0), end, end + 0.12, 'result');
			}
			const chosen = el(svg, 'path', { d: bent(w[0], judge, 0), class: 'd-chosen', opacity: 0 });
			el(chosen, 'animate', {
				attributeName: 'opacity',
				dur: `${cycle}s`,
				repeatCount: 'indefinite',
				values: '0;0;1;1;0;0',
				keyTimes: times([0, 0.6, 0.64, 0.9, 0.94, 1])
			});
			glow(svg, cycle, w[0], 10, 0.6, 0.94, 'decision');
			pulse(svg, cycle, judge, 0.6, 9, 20);
		}
	},
	pipeline: {
		cycle: 8,
		still: 0.5,
		draw(svg, cycle) {
			const W = 84;
			const Y = 62;
			const H = 52;
			const rest = 100;
			const xs = [48, 188];
			const boxes = xs.map((x) => {
				el(svg, 'rect', { x, y: Y, width: W, height: H, class: 'd-stage' });
				return P(x + W / 2, rest);
			});
			const x1 = xs[0] + W + 6;
			const x2 = xs[1] - 6;
			el(svg, 'line', { x1, y1: 88, x2, y2: 88, class: 'd-arrow' });
			el(svg, 'path', { d: `M${x2 - 5} 84 L${x2} 88 L${x2 - 5} 92`, class: 'd-arrow' });
			const active: [number, number][] = [
				[0.04, 0.5],
				[0.6, 0.94]
			];
			xs.forEach((x, i) => {
				const hl = el(svg, 'rect', {
					x,
					y: Y,
					width: W,
					height: H,
					class: 'd-stage-active',
					opacity: 0
				});
				el(hl, 'animate', {
					attributeName: 'opacity',
					dur: `${cycle}s`,
					repeatCount: 'indefinite',
					values: '0;0;1;1;0;0',
					keyTimes: times([
						0,
						active[i][0],
						active[i][0] + 0.02,
						active[i][1] - 0.02,
						active[i][1],
						1
					])
				});
			});
			const hold = (a: P, t0: number, t1: number) =>
				dot(svg, cycle, `M${a.x} ${a.y} L${a.x + 0.01} ${a.y}`, t0, t1, 'result', 3);
			const move = (a: P, b: P, t0: number, t1: number) =>
				dot(svg, cycle, `M${a.x} ${a.y} L${b.x} ${b.y}`, t0, t1, 'result', 3);
			move(P(-6, rest), boxes[0], 0, 0.06);
			hold(boxes[0], 0.06, 0.5);
			// The draft needs one approval before the next step is added.
			const reviewer = P(boxes[0].x, 150);
			node(svg, reviewer, 6);
			dot(svg, cycle, bent(reviewer, boxes[0], 0), 0.3, 0.44, 'decision', 2.4);
			pulse(svg, cycle, boxes[0], 0.44, 6, 16);
			move(boxes[0], boxes[1], 0.5, 0.6);
			hold(boxes[1], 0.6, 0.94);
		}
	}
};

/** Draws a way of working's picture into an empty SVG with a 320×180 view box. */
export function drawDiagram(svg: Svg, id: string) {
	const drawing = DRAWINGS[id];
	if (!drawing) return;
	svg.replaceChildren();
	drawing.draw(svg, drawing.cycle);
	if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
		svg.pauseAnimations();
		svg.setCurrentTime(drawing.cycle * drawing.still);
	}
}
