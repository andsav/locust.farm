/**
 * The swarm backdrop: agents chasing a goal across a canvas, and a locust they
 * push around as they pass. This module owns the frame loop and everything that
 * touches the page; the drawing is in `renderer.ts`.
 */

import { createLocust, seedLife, stepLife } from './locust.ts';
import { SwarmRenderer, type Palette, type Rgb } from './renderer.ts';

/** What the swarm chases: a wandering point, the cursor, or the last place clicked. */
export type GoalMode = 'wander' | 'cursor' | 'click';

export interface SwarmOptions {
	/** Number of agents. */
	agents: number;
	goalMode: GoalMode;
	/** How much of each trail fades per step, from 0 (never fades) to 1 (no trail). */
	trail: number;
	/**
	 * How much harder agents are pulled toward the cursor for every 100 pixels they
	 * are from it, so the swarm keeps up when the cursor moves away. 0 is the same
	 * pull at any distance; above about 0.4 the swarm overshoots.
	 */
	catchUp: number;
}

export interface Swarm {
	/** Applies new options from the next frame on. */
	update(options: SwarmOptions): void;
	/** Stops the animation and releases its listeners and GPU resources. */
	destroy(): void;
}

/**
 * The simulation advances in fixed steps, so it moves at the same speed on every
 * display. The design was tuned with one step per frame on a 120 Hz screen.
 */
const STEPS_PER_SECOND = 120;
/** Steps to catch up on after a slow frame before the simulation falls behind instead. */
const MAX_STEPS_PER_FRAME = 4;
const MAX_PIXEL_RATIO = 2;
/**
 * Largest drawing buffer, in device pixels: that of a 4K screen. Every step fills
 * the buffer twice, so a larger canvas is drawn at a lower pixel ratio instead.
 */
const MAX_PIXELS = 3840 * 2160;
/** Draws after a resize, enough to rebuild the opacity that accumulates over steps. */
const REDRAW_PASSES = 5;
const LIFE_INTERVAL_MS = 280;
/** Fraction of the remaining distance the goal covers per step. */
const WANDER_EASE = 0.06;
/** The goal stays close to the cursor, so the swarm responds to it at once. */
const CURSOR_EASE = 0.2;
/** With reduced motion, the swarm is shown as a still taken this far into the simulation. */
const STILL_STEPS = 360;
/** Steps drawn for a still, enough for trails to reach full length. */
const STILL_TRAIL_STEPS = 24;
/** How long the size must hold before a still is redrawn, so a drag redraws it once. */
const STILL_SETTLE_MS = 150;

/**
 * Starts the swarm on a canvas. Returns nothing where WebGL2 is unavailable, would
 * run in software, or fails to set up; the canvas then stays empty and shows the
 * page background.
 */
export function createSwarm(canvas: HTMLCanvasElement, initial: SwarmOptions): Swarm | undefined {
	const gl = canvas.getContext('webgl2', {
		alpha: false,
		antialias: false,
		depth: false,
		stencil: false,
		failIfMajorPerformanceCaveat: true
	});
	if (!gl) return;

	const palette = readPalette(canvas);
	const locust = createLocust();
	const alive = seedLife(locust);
	const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
	const listeners = new AbortController();
	const { signal } = listeners;

	let options = initial;
	let renderer: SwarmRenderer | undefined;
	let width = 0;
	let height = 0;
	let pixelRatio = 0;
	let agents = -1;

	/** Whether the reduced-motion still has been through its warm-up. */
	let settled = false;
	let resizedAt = 0;
	let frameRequest = 0;
	let lastFrame = 0;
	let pendingSteps = 0;
	let lastGeneration = 0;

	// Positions are in CSS pixels relative to the canvas.
	let wander = 0;
	const goal = { x: 0, y: 0, placed: false };
	const pointer = { x: 0, y: 0, present: false };
	const pinned = { x: 0, y: 0, present: false };

	/** Creates the renderer. Reports a failure and returns false, leaving the canvas empty. */
	const build = () => {
		try {
			renderer = new SwarmRenderer(gl, locust, palette);
		} catch (error) {
			console.error(error);
			renderer = undefined;
			return false;
		}
		renderer.setAlive(alive);
		pixelRatio = 0;
		agents = -1;
		settled = false;
		resize();
		return true;
	};

	const resize = () => {
		const box = canvas.getBoundingClientRect();
		if (!renderer || !box.width || !box.height) return;
		const ratio = Math.min(
			window.devicePixelRatio || 1,
			MAX_PIXEL_RATIO,
			Math.sqrt(MAX_PIXELS / (box.width * box.height))
		);
		if (box.width === width && box.height === height && ratio === pixelRatio) return;
		({ width, height } = box);
		pixelRatio = ratio;
		renderer.resize(width, height, pixelRatio);
		resizedAt = performance.now();
		if (!goal.placed) {
			goal.x = width / 2;
			goal.y = height / 2;
			goal.placed = true;
		}
		// Resize observers run after animation frames, so without a redraw here the
		// browser would paint the cleared canvas.
		for (let i = 0; i < REDRAW_PASSES; i++) {
			renderer.fade(options.trail);
			renderer.draw(goal.x, goal.y);
		}
		renderer.present();
		schedule();
	};

	/** One simulation step: moves the goal, then the swarm. */
	const simulate = () => {
		if (!renderer) return;
		const { goalMode, catchUp } = options;
		wander += 0.004;
		let targetX = width * 0.5 + Math.cos(wander * 1.3) * width * 0.28;
		let targetY = height * 0.38 + Math.sin(wander * 2.1) * height * 0.18;
		const following = goalMode === 'cursor' && pointer.present;
		if (following) {
			targetX = pointer.x;
			targetY = pointer.y;
		} else if (goalMode === 'click') {
			targetX = pinned.present ? pinned.x : width * 0.62;
			targetY = pinned.present ? pinned.y : height * 0.36;
		}
		const ease = following ? CURSOR_EASE : WANDER_EASE;
		goal.x += (targetX - goal.x) * ease;
		goal.y += (targetY - goal.y) * ease;

		// The wandering goal keeps the design's motion; only the cursor is chased harder.
		renderer.simulate(goal.x, goal.y, following ? catchUp / 100 : 0);
	};

	/**
	 * Simulates `steps` steps and draws each into the scene. Fading the scene is the
	 * costly part of a step, so steps are drawn in pairs that share one fade: the
	 * earlier step of a pair is drawn as it would look after the later step's fade.
	 */
	const advance = (steps: number) => {
		if (!renderer) return;
		const keep = 1 - options.trail;
		for (let left = steps; left > 0; left -= 2) {
			simulate();
			if (left === 1) {
				renderer.fade(options.trail);
			} else {
				const { x, y } = goal;
				simulate();
				renderer.fade(1 - keep * keep);
				renderer.draw(x, y, keep, true);
			}
			renderer.draw(goal.x, goal.y);
		}
	};

	const frame = (now: number) => {
		frameRequest = 0;
		if (!renderer || !width) return;
		if (options.agents !== agents) {
			renderer.setAgents((agents = options.agents));
			settled = false;
		}

		if (reducedMotion.matches) {
			if (settled && now - resizedAt < STILL_SETTLE_MS) return schedule();
			// The first still lets the swarm gather; later ones only redraw its trails.
			for (let i = settled ? 0 : STILL_STEPS - STILL_TRAIL_STEPS; i > 0; i--) simulate();
			advance(STILL_TRAIL_STEPS);
			renderer.present();
			settled = true;
			return;
		}
		schedule();

		if (now - lastGeneration > LIFE_INTERVAL_MS && renderer.locustVisible) {
			lastGeneration = now;
			stepLife(locust, alive);
			renderer.setAlive(alive);
		}

		const elapsed = lastFrame ? now - lastFrame : 1000 / STEPS_PER_SECOND;
		lastFrame = now;
		pendingSteps += (elapsed * STEPS_PER_SECOND) / 1000;
		const steps = Math.min(Math.round(pendingSteps), MAX_STEPS_PER_FRAME);
		pendingSteps -= steps;
		// A backlog is dropped, so the simulation falls behind rather than racing to catch up.
		if (pendingSteps > 0.5) pendingSteps = 0;
		if (!steps) return;

		advance(steps);
		renderer.present();
	};

	const schedule = () => {
		frameRequest ||= requestAnimationFrame(frame);
	};

	const place = (point: typeof pointer, event: MouseEvent) => {
		const box = canvas.getBoundingClientRect();
		point.x = event.clientX - box.left;
		point.y = event.clientY - box.top;
		point.present = true;
	};

	// The canvas sits behind the page content, so the pointer is tracked on the window.
	window.addEventListener(
		'pointermove',
		(event) => {
			if (options.goalMode === 'cursor') place(pointer, event);
		},
		{ signal, passive: true }
	);
	document.documentElement.addEventListener('pointerleave', () => (pointer.present = false), {
		signal
	});
	window.addEventListener(
		'click',
		(event) => {
			const onControl = event.target instanceof Element && event.target.closest('a, button');
			if (options.goalMode === 'click' && !onControl) place(pinned, event);
		},
		{ signal }
	);

	canvas.addEventListener(
		'webglcontextlost',
		(event) => {
			// Without this the browser never restores the context.
			event.preventDefault();
			cancelAnimationFrame(frameRequest);
			frameRequest = 0;
			renderer = undefined;
		},
		{ signal }
	);
	canvas.addEventListener('webglcontextrestored', build, { signal });

	const restart = () => {
		lastFrame = 0;
		pendingSteps = 0;
		schedule();
	};
	reducedMotion.addEventListener('change', restart, { signal });

	const observer = new ResizeObserver(resize);
	try {
		// Also fires when only the device pixel ratio changes, such as on a move between screens.
		observer.observe(canvas, { box: 'device-pixel-content-box' });
	} catch {
		observer.observe(canvas);
		const watchPixelRatio = () => {
			const query = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
			const changed = () => {
				resize();
				watchPixelRatio();
			};
			query.addEventListener('change', changed, { once: true, signal });
		};
		watchPixelRatio();
	}

	if (!build()) {
		observer.disconnect();
		listeners.abort();
		return;
	}

	return {
		update(next) {
			options = next;
			if (next.goalMode !== 'cursor') pointer.present = false;
			// A still frame has to be redrawn to show the change.
			schedule();
		},
		destroy() {
			cancelAnimationFrame(frameRequest);
			observer.disconnect();
			listeners.abort();
			renderer?.dispose();
			renderer = undefined;
		}
	};
}

/** Reads the swarm colors from the design tokens in effect on the canvas. */
function readPalette(canvas: HTMLCanvasElement): Palette {
	const style = getComputedStyle(canvas);
	// A 2D context normalises any opaque CSS color to #rrggbb, whatever form a minifier leaves it in.
	const parser = document.createElement('canvas').getContext('2d')!;
	const color = (token: string): Rgb => {
		parser.fillStyle = style.getPropertyValue(token);
		const value = parseInt(parser.fillStyle.slice(1), 16);
		return [(value >> 16) / 255, ((value >> 8) & 0xff) / 255, (value & 0xff) / 255];
	};
	return {
		background: color('--swarm-bg'),
		agent: color('--swarm-agent'),
		goal: color('--swarm-goal'),
		cellBody: color('--swarm-cell-body'),
		cellLeg: color('--swarm-cell-leg'),
		cellEye: color('--swarm-cell-eye')
	};
}
