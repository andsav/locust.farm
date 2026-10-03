/**
 * WebGL2 renderer for the swarm.
 *
 * Agents and locust cells live in vertex buffers and are advanced on the GPU with
 * transform feedback, so a step costs the main thread a fixed handful of draw calls
 * however many agents there are. Each kind of state has two buffers: a step reads
 * one and writes the other, then they swap.
 *
 * One step:
 *   1. Cells move, pushed by the agent density of the previous step.
 *   2. Agents are counted into the density grid.
 *   3. Agents move.
 *   4. The scene is faded, which leaves trails, and the new state is drawn over it.
 * The scene is kept in a texture and copied to the canvas by `present`.
 */

import {
	bindAttributes,
	createPrograms,
	createTarget,
	deleteTarget,
	type Program,
	type Target
} from './gl.ts';
import { layoutLocust, type Locust, type LocustLayout } from './locust.ts';
import * as shaders from './shaders.ts';

export type Rgb = readonly [red: number, green: number, blue: number];

export interface Palette {
	background: Rgb;
	agent: Rgb;
	goal: Rgb;
	cellBody: Rgb;
	cellLeg: Rgb;
	cellEye: Rgb;
}

/** Side of a density grid cell, in CSS pixels. */
const DENSITY_CELL = 24;
const AGENT_OPACITY = 0.75;
const GOAL_RADIUS = 3.5;

const FLOAT = Float32Array.BYTES_PER_ELEMENT;
/** Agent state: position and velocity. */
const AGENT_STRIDE = 4 * FLOAT;
/** Cell state: position, velocity and brightness. */
const CELL_STRIDE = 5 * FLOAT;
/** Fixed cell data: grid column, grid row and kind. */
const CELL_INFO_STRIDE = 3 * FLOAT;

// Attribute locations, matching the `layout(location = …)` qualifiers in the shaders.
const POSITION = 0;
const VELOCITY = 1;
const AGENT_SPEED = 2;
const CELL_BRIGHTNESS = 2;
const CELL_GRID = 3;
const CELL_ALIVE = 4;
const CELL_KIND = 5;

/** The two copies of a simulated buffer, with the objects that read and write each. */
interface Flock {
	state: WebGLBuffer[];
	/** Reads `state[i]` to compute the next step. */
	update: WebGLVertexArrayObject[];
	/** Reads `state[i]` as one instance per element. */
	draw: WebGLVertexArrayObject[];
	/** Captures the next step into `state[i]`. */
	feedback: WebGLTransformFeedback[];
	/** Index of the current state. */
	current: number;
}

export class SwarmRenderer {
	readonly #gl: WebGL2RenderingContext;
	readonly #palette: Palette;
	readonly #locust: Locust;
	readonly #programs: Record<
		| 'agentUpdate'
		| 'density'
		| 'cellUpdate'
		| 'fill'
		| 'cellDraw'
		| 'streakCoverage'
		| 'streakInk'
		| 'dot',
		Program
	>;

	readonly #agents: Flock;
	readonly #agentSpeed: WebGLBuffer;
	#agentCount = 0;

	readonly #cells: Flock;
	readonly #cellInfo: WebGLBuffer;
	readonly #cellAlive: WebGLBuffer;

	/** Used by draws whose vertices come entirely from `gl_VertexID`. */
	readonly #noAttributes: WebGLVertexArrayObject;

	#width = 0;
	#height = 0;
	#layout: LocustLayout = layoutLocust(0, 0);
	#scene: Target | undefined;
	#density: Target | undefined;
	#step = 0;

	constructor(gl: WebGL2RenderingContext, locust: Locust, palette: Palette) {
		this.#gl = gl;
		this.#locust = locust;
		this.#palette = palette;

		this.#programs = createPrograms(gl, {
			agentUpdate: {
				vertex: shaders.AGENT_UPDATE_VERTEX,
				fragment: shaders.DISCARD_FRAGMENT,
				feedback: ['v_position', 'v_velocity']
			},
			density: { vertex: shaders.DENSITY_VERTEX, fragment: shaders.DENSITY_FRAGMENT },
			cellUpdate: {
				vertex: shaders.CELL_UPDATE_VERTEX,
				fragment: shaders.DISCARD_FRAGMENT,
				feedback: ['v_position', 'v_velocity', 'v_brightness']
			},
			fill: { vertex: shaders.FILL_VERTEX, fragment: shaders.FILL_FRAGMENT },
			cellDraw: { vertex: shaders.CELL_DRAW_VERTEX, fragment: shaders.CELL_DRAW_FRAGMENT },
			streakCoverage: {
				vertex: shaders.STREAK_VERTEX,
				fragment: shaders.STREAK_COVERAGE_FRAGMENT
			},
			streakInk: { vertex: shaders.STREAK_VERTEX, fragment: shaders.STREAK_INK_FRAGMENT },
			dot: { vertex: shaders.DOT_VERTEX, fragment: shaders.DOT_FRAGMENT }
		});

		this.#noAttributes = gl.createVertexArray();

		this.#agentSpeed = gl.createBuffer();
		this.#agents = this.#createFlock(
			AGENT_STRIDE,
			[
				{ location: POSITION, size: 2 },
				{ location: VELOCITY, size: 2, offset: 2 * FLOAT }
			],
			() => bindAttributes(gl, this.#agentSpeed, FLOAT, [{ location: AGENT_SPEED, size: 1 }]),
			() => {}
		);

		this.#cellInfo = gl.createBuffer();
		this.#cellAlive = gl.createBuffer();
		this.#cells = this.#createFlock(
			CELL_STRIDE,
			[
				{ location: POSITION, size: 2 },
				{ location: VELOCITY, size: 2, offset: 2 * FLOAT },
				{ location: CELL_BRIGHTNESS, size: 1, offset: 4 * FLOAT }
			],
			() => {
				bindAttributes(gl, this.#cellInfo, CELL_INFO_STRIDE, [{ location: CELL_GRID, size: 2 }]);
				bindAttributes(gl, this.#cellAlive, 1, [
					{ location: CELL_ALIVE, size: 1, type: gl.UNSIGNED_BYTE }
				]);
			},
			() =>
				bindAttributes(
					gl,
					this.#cellInfo,
					CELL_INFO_STRIDE,
					[{ location: CELL_KIND, size: 1, offset: 2 * FLOAT }],
					true
				)
		);

		const info = new Float32Array(locust.count * 3);
		for (let cell = 0; cell < locust.count; cell++) {
			info[cell * 3] = locust.grid[cell * 2];
			info[cell * 3 + 1] = locust.grid[cell * 2 + 1];
			info[cell * 3 + 2] = locust.kind[cell];
		}
		gl.bindBuffer(gl.ARRAY_BUFFER, this.#cellInfo);
		gl.bufferData(gl.ARRAY_BUFFER, info, gl.STATIC_DRAW);
		gl.bindBuffer(gl.ARRAY_BUFFER, this.#cellAlive);
		gl.bufferData(gl.ARRAY_BUFFER, locust.count, gl.DYNAMIC_DRAW);
		gl.bindBuffer(gl.ARRAY_BUFFER, null);

		gl.disable(gl.DEPTH_TEST);
		gl.disable(gl.DITHER);
		gl.enable(gl.BLEND);

		const { density, cellUpdate, fill, cellDraw, streakCoverage, streakInk, dot } = this.#programs;
		gl.useProgram(density.handle);
		gl.uniform1f(density.uniforms.u_cellSize, DENSITY_CELL);
		gl.useProgram(cellUpdate.handle);
		gl.uniform1f(cellUpdate.uniforms.u_cellSize, DENSITY_CELL);
		gl.uniform1i(cellUpdate.uniforms.u_density, 0);
		gl.useProgram(cellDraw.handle);
		gl.uniform3fv(cellDraw.uniforms.u_colors, [
			...palette.cellBody,
			...palette.cellLeg,
			...palette.cellEye
		]);
		gl.useProgram(fill.handle);
		gl.uniform4f(fill.uniforms.u_color, ...palette.background, 1);
		gl.useProgram(streakCoverage.handle);
		gl.uniform1f(streakCoverage.uniforms.u_opacity, AGENT_OPACITY);
		gl.useProgram(streakInk.handle);
		gl.uniform3f(streakInk.uniforms.u_color, ...palette.agent);
		gl.useProgram(dot.handle);
		gl.uniform3f(dot.uniforms.u_color, ...palette.goal);
		gl.uniform1f(dot.uniforms.u_radius, GOAL_RADIUS);
		for (const program of [cellDraw, streakInk, dot]) {
			gl.useProgram(program.handle);
			gl.uniform3f(program.uniforms.u_background, ...palette.background);
		}
	}

	/**
	 * Sizes the canvas to `width` × `height` CSS pixels at the given pixel ratio.
	 * Trails are cleared; agents and cells keep their positions. The canvas is left
	 * blank, so the caller should `draw` and `present` before the browser paints.
	 */
	resize(width: number, height: number, pixelRatio: number): void {
		const gl = this.#gl;
		gl.canvas.width = Math.max(1, Math.round(width * pixelRatio));
		gl.canvas.height = Math.max(1, Math.round(height * pixelRatio));
		// The browser may allocate a smaller buffer than requested, so measure the real ratio.
		pixelRatio = gl.drawingBufferWidth / width;

		const wasHidden = this.#layout.hidden;
		this.#width = width;
		this.#height = height;
		this.#layout = layoutLocust(width, height);

		deleteTarget(gl, this.#scene);
		deleteTarget(gl, this.#density);
		this.#scene = createTarget(gl, gl.drawingBufferWidth, gl.drawingBufferHeight);
		this.#density = createTarget(
			gl,
			Math.ceil(width / DENSITY_CELL) + 1,
			Math.ceil(height / DENSITY_CELL) + 1
		);
		// The density grid starts empty because new textures are zeroed.
		gl.bindFramebuffer(gl.FRAMEBUFFER, this.#scene.framebuffer);
		gl.clearColor(...this.#palette.background, 1);
		gl.clear(gl.COLOR_BUFFER_BIT);

		const { density, cellUpdate, cellDraw, streakCoverage, streakInk, dot } = this.#programs;
		const { pitch, originX, originY } = this.#layout;
		gl.useProgram(density.handle);
		gl.uniform2f(density.uniforms.u_gridSize, this.#density.width, this.#density.height);
		gl.useProgram(cellUpdate.handle);
		gl.uniform2f(cellUpdate.uniforms.u_origin, originX, originY);
		gl.uniform1f(cellUpdate.uniforms.u_pitch, pitch);
		gl.useProgram(cellDraw.handle);
		gl.uniform1f(cellDraw.uniforms.u_size, Math.max(1, pitch - 1));
		for (const program of [cellDraw, streakCoverage, streakInk, dot]) {
			gl.useProgram(program.handle);
			gl.uniform2f(program.uniforms.u_view, 2 / width, -2 / height);
			gl.uniform1f(program.uniforms.u_pixelRatio, pixelRatio);
		}

		// Cells are frozen while hidden, so on reappearing they start from their new places.
		if (wasHidden && !this.#layout.hidden) this.#placeCells();
	}

	/** Whether the locust is shown at the current size. While hidden it is not simulated. */
	get locustVisible(): boolean {
		return !this.#layout.hidden;
	}

	/** Replaces the swarm with `count` agents scattered across the canvas, at rest. */
	setAgents(count: number): void {
		const gl = this.#gl;
		const state = new Float32Array(count * 4);
		const speed = new Float32Array(count);
		for (let agent = 0; agent < count; agent++) {
			state[agent * 4] = Math.random() * this.#width;
			state[agent * 4 + 1] = Math.random() * this.#height;
			speed[agent] = 0.6 + Math.random() * 0.8;
		}
		for (const buffer of this.#agents.state) {
			gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
			gl.bufferData(gl.ARRAY_BUFFER, state, gl.DYNAMIC_COPY);
		}
		gl.bindBuffer(gl.ARRAY_BUFFER, this.#agentSpeed);
		gl.bufferData(gl.ARRAY_BUFFER, speed, gl.STATIC_DRAW);
		gl.bindBuffer(gl.ARRAY_BUFFER, null);
		this.#agentCount = count;
	}

	/** Uploads the Game of Life generation: one byte per cell, 1 if alive. */
	setAlive(alive: Uint8Array): void {
		const gl = this.#gl;
		gl.bindBuffer(gl.ARRAY_BUFFER, this.#cellAlive);
		gl.bufferSubData(gl.ARRAY_BUFFER, 0, alive);
		gl.bindBuffer(gl.ARRAY_BUFFER, null);
	}

	/**
	 * Advances agents and cells by one step, with the goal at the given CSS pixel
	 * position. `catchUp` is the extra pull on an agent per CSS pixel of distance from
	 * the goal, as a fraction of its normal pull.
	 */
	simulate(goalX: number, goalY: number, catchUp: number): void {
		const gl = this.#gl;
		const { agentUpdate, density, cellUpdate } = this.#programs;
		const agents = this.#agents;
		const cells = this.#cells;
		const grid = this.#density;
		if (!grid) return;

		if (!this.#layout.hidden) {
			gl.bindFramebuffer(gl.FRAMEBUFFER, null);
			gl.useProgram(cellUpdate.handle);
			gl.activeTexture(gl.TEXTURE0);
			gl.bindTexture(gl.TEXTURE_2D, grid.texture);
			this.#transform(cells, this.#locust.count);
			gl.bindTexture(gl.TEXTURE_2D, null);

			// Only the cells read the density grid, so it is not built while they are hidden.
			gl.bindFramebuffer(gl.FRAMEBUFFER, grid.framebuffer);
			gl.viewport(0, 0, grid.width, grid.height);
			gl.clearColor(0, 0, 0, 0);
			gl.clear(gl.COLOR_BUFFER_BIT);
			gl.blendFunc(gl.ONE, gl.ONE);
			gl.useProgram(density.handle);
			gl.bindVertexArray(agents.update[agents.current]);
			gl.drawArrays(gl.POINTS, 0, this.#agentCount);
		}

		gl.useProgram(agentUpdate.handle);
		gl.uniform2f(agentUpdate.uniforms.u_goal, goalX, goalY);
		gl.uniform1f(agentUpdate.uniforms.u_catchUp, catchUp);
		gl.uniform1ui(agentUpdate.uniforms.u_step, this.#step++);
		this.#transform(agents, this.#agentCount);
	}

	/** Fades the scene toward the background by `amount`: 0 keeps it all, 1 clears it. */
	fade(amount: number): void {
		const gl = this.#gl;
		const scene = this.#scene;
		if (!scene) return;

		gl.bindFramebuffer(gl.FRAMEBUFFER, scene.framebuffer);
		gl.viewport(0, 0, scene.width, scene.height);
		// The amount is the blend constant, which leaves the fill's own alpha free to
		// reset the scene's alpha to 1.
		gl.blendColor(0, 0, 0, amount);
		gl.blendFuncSeparate(gl.CONSTANT_ALPHA, gl.ONE_MINUS_CONSTANT_ALPHA, gl.ONE, gl.ZERO);
		gl.useProgram(this.#programs.fill.handle);
		gl.bindVertexArray(this.#noAttributes);
		gl.drawArrays(gl.TRIANGLES, 0, 3);
	}

	/**
	 * Draws the state over the scene, with the goal at the given position.
	 *
	 * To draw two steps under one fade, draw the earlier one with `previous` set and
	 * `keep` at the share of it that the later step's fade would have left. It then
	 * looks exactly as if it had been drawn at full strength and faded.
	 */
	draw(goalX: number, goalY: number, keep = 1, previous = false): void {
		const gl = this.#gl;
		const { cellDraw, streakCoverage, streakInk, dot } = this.#programs;
		const scene = this.#scene;
		const agents = this.#agents;
		const cells = this.#cells;
		if (!scene) return;

		gl.bindFramebuffer(gl.FRAMEBUFFER, scene.framebuffer);
		gl.viewport(0, 0, scene.width, scene.height);

		// Cells and the goal blend by their own alpha and leave the scene's alpha alone.
		gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ZERO, gl.ONE);
		if (!this.#layout.hidden) {
			gl.useProgram(cellDraw.handle);
			gl.uniform1f(cellDraw.uniforms.u_keep, keep);
			gl.bindVertexArray(cells.draw[previous ? 1 - cells.current : cells.current]);
			gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, this.#locust.count);
		}

		// Streaks, first as coverage in the alpha channel, keeping the strongest per pixel…
		gl.bindVertexArray(agents.draw[previous ? 1 - agents.current : agents.current]);
		gl.colorMask(false, false, false, true);
		gl.blendEquationSeparate(gl.FUNC_ADD, gl.MIN);
		gl.useProgram(streakCoverage.handle);
		gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, this.#agentCount);
		// …then as ink weighted by that coverage. Inking a pixel restores its alpha to 1,
		// so any other streak over the same pixel leaves it unchanged.
		gl.colorMask(true, true, true, true);
		gl.blendEquation(gl.FUNC_ADD);
		gl.blendFuncSeparate(gl.ONE_MINUS_DST_ALPHA, gl.DST_ALPHA, gl.ONE, gl.ZERO);
		gl.useProgram(streakInk.handle);
		gl.uniform1f(streakInk.uniforms.u_keep, keep);
		gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, this.#agentCount);

		gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ZERO, gl.ONE);
		gl.useProgram(dot.handle);
		gl.uniform1f(dot.uniforms.u_keep, keep);
		gl.uniform2f(dot.uniforms.u_center, goalX, goalY);
		gl.bindVertexArray(this.#noAttributes);
		gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
	}

	/** Copies the scene to the canvas. */
	present(): void {
		const gl = this.#gl;
		const scene = this.#scene;
		if (!scene) return;
		const { width, height } = scene;
		gl.bindFramebuffer(gl.READ_FRAMEBUFFER, scene.framebuffer);
		gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
		gl.blitFramebuffer(0, 0, width, height, 0, 0, width, height, gl.COLOR_BUFFER_BIT, gl.NEAREST);
	}

	dispose(): void {
		const gl = this.#gl;
		deleteTarget(gl, this.#scene);
		deleteTarget(gl, this.#density);
		for (const flock of [this.#agents, this.#cells]) {
			flock.state.forEach((buffer) => gl.deleteBuffer(buffer));
			[...flock.update, ...flock.draw].forEach((vertexArray) => gl.deleteVertexArray(vertexArray));
			flock.feedback.forEach((feedback) => gl.deleteTransformFeedback(feedback));
		}
		for (const buffer of [this.#agentSpeed, this.#cellInfo, this.#cellAlive])
			gl.deleteBuffer(buffer);
		gl.deleteVertexArray(this.#noAttributes);
		for (const program of Object.values(this.#programs)) gl.deleteProgram(program.handle);
	}

	/**
	 * Creates the two state buffers of a simulated set and, for each, a vertex array
	 * for the update pass, one for instanced drawing, and a transform feedback target.
	 * The callbacks add the attributes that do not come from the state buffer.
	 */
	#createFlock(
		stride: number,
		attributes: Parameters<typeof bindAttributes>[3],
		bindUpdateExtras: () => void,
		bindDrawExtras: () => void
	): Flock {
		const gl = this.#gl;
		const flock: Flock = { state: [], update: [], draw: [], feedback: [], current: 0 };
		for (let i = 0; i < 2; i++) {
			const state = gl.createBuffer();

			const update = gl.createVertexArray();
			gl.bindVertexArray(update);
			bindAttributes(gl, state, stride, attributes);
			bindUpdateExtras();

			const draw = gl.createVertexArray();
			gl.bindVertexArray(draw);
			bindAttributes(gl, state, stride, attributes, true);
			bindDrawExtras();
			gl.bindVertexArray(null);

			const feedback = gl.createTransformFeedback();
			gl.bindTransformFeedback(gl.TRANSFORM_FEEDBACK, feedback);
			gl.bindBufferBase(gl.TRANSFORM_FEEDBACK_BUFFER, 0, state);
			gl.bindTransformFeedback(gl.TRANSFORM_FEEDBACK, null);
			// bindBufferBase also sets the generic binding, which is not part of the feedback object.
			gl.bindBuffer(gl.TRANSFORM_FEEDBACK_BUFFER, null);

			flock.state.push(state);
			flock.update.push(update);
			flock.draw.push(draw);
			flock.feedback.push(feedback);
		}
		return flock;
	}

	/** Runs the bound update program over a flock, writing its next state, then swaps. */
	#transform(flock: Flock, count: number): void {
		const gl = this.#gl;
		const next = 1 - flock.current;
		gl.enable(gl.RASTERIZER_DISCARD);
		gl.bindVertexArray(flock.update[flock.current]);
		gl.bindTransformFeedback(gl.TRANSFORM_FEEDBACK, flock.feedback[next]);
		gl.beginTransformFeedback(gl.POINTS);
		gl.drawArrays(gl.POINTS, 0, count);
		gl.endTransformFeedback();
		gl.bindTransformFeedback(gl.TRANSFORM_FEEDBACK, null);
		gl.bindVertexArray(null);
		gl.disable(gl.RASTERIZER_DISCARD);
		flock.current = next;
	}

	/** Puts every cell at rest at its place in the silhouette, half lit. */
	#placeCells(): void {
		const gl = this.#gl;
		const { count, grid } = this.#locust;
		const { pitch, originX, originY } = this.#layout;
		const state = new Float32Array(count * 5);
		for (let cell = 0; cell < count; cell++) {
			state[cell * 5] = originX + grid[cell * 2] * pitch;
			state[cell * 5 + 1] = originY + grid[cell * 2 + 1] * pitch;
			state[cell * 5 + 4] = 0.5;
		}
		for (const buffer of this.#cells.state) {
			gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
			gl.bufferData(gl.ARRAY_BUFFER, state, gl.DYNAMIC_COPY);
		}
		gl.bindBuffer(gl.ARRAY_BUFFER, null);
	}
}
