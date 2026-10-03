/**
 * The pixel-art locust behind the hero: a silhouette on a 58×29 grid whose cells
 * flicker according to a Game of Life confined to the silhouette.
 *
 * Everything here is plain data and runs on the CPU. The renderer owns the cells'
 * positions, which live on the GPU.
 */

export const LOCUST_COLS = 58;
export const LOCUST_ROWS = 29;

/** Mask values. Cells are drawn in this order, so later kinds paint over earlier ones. */
export const BODY = 1;
export const LEG = 2;
export const EYE = 3;

const LIFE_INITIAL_ALIVE = 0.35;
const LIFE_SPONTANEOUS_BIRTH = 0.004;
const LIFE_RESEED_BELOW = 0.08;
const LIFE_RESEED_CHANCE = 0.3;

export interface Locust {
	/** Number of cells in the silhouette. */
	readonly count: number;
	/** Column and row of each cell, interleaved. Cells are ordered by kind. */
	readonly grid: Float32Array;
	/** `BODY`, `LEG` or `EYE` for each cell. */
	readonly kind: Uint8Array;
	/** The neighbours of cell `i` are `neighbors[neighborStart[i]]` up to `neighbors[neighborStart[i + 1]]`. */
	readonly neighborStart: Uint32Array;
	readonly neighbors: Uint32Array;
}

/** Rasterises the silhouette. Returns one mask value per grid position, 0 where empty. */
export function buildLocustMask(): Uint8Array {
	const cols = LOCUST_COLS;
	const rows = LOCUST_ROWS;
	const body = new Uint8Array(cols * rows);
	const leg = new Uint8Array(cols * rows);

	const disc = (mask: Uint8Array, cx: number, cy: number, r: number) => {
		for (let y = Math.floor(cy - r - 1); y <= cy + r + 1; y++) {
			for (let x = Math.floor(cx - r - 1); x <= cx + r + 1; x++) {
				if (x < 0 || y < 0 || x >= cols || y >= rows) continue;
				if ((x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2 <= r * r) mask[y * cols + x] = 1;
			}
		}
	};
	const ellipse = (cx: number, cy: number, rx: number, ry: number) => {
		for (let y = 0; y < rows; y++) {
			for (let x = 0; x < cols; x++) {
				if (((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2 <= 1) body[y * cols + x] = 1;
			}
		}
	};
	/** A stroke from one point to another whose radius tapers from `r0` to `r1`. */
	const limb = (
		mask: Uint8Array,
		x0: number,
		y0: number,
		x1: number,
		y1: number,
		r0: number,
		r1: number
	) => {
		const steps = Math.ceil(Math.hypot(x1 - x0, y1 - y0) * 3);
		for (let i = 0; i <= steps; i++) {
			const t = i / steps;
			disc(mask, x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, r0 + (r1 - r0) * t);
		}
	};

	// Head and mandible.
	ellipse(9, 15, 4.5, 5.5);
	disc(body, 7, 20, 1.8);
	// Thorax, abdomen and wing.
	ellipse(16, 13.5, 5, 5.5);
	limb(body, 20, 14, 54, 14.5, 5, 1.5);
	limb(body, 18, 11.5, 55, 12.5, 2.5, 1);
	// Antennae.
	limb(body, 8, 10, 4, 4, 0.6, 0.6);
	limb(body, 4, 4, 1, 1, 0.6, 0.6);
	limb(body, 11, 10, 9, 5, 0.6, 0.6);
	limb(body, 9, 5, 7, 2, 0.6, 0.6);
	// Front and middle legs.
	limb(body, 12, 19, 10, 25, 0.8, 0.8);
	limb(body, 10, 25, 7, 26, 0.7, 0.7);
	limb(body, 19, 19, 21, 25, 0.8, 0.8);
	limb(body, 21, 25, 24, 26, 0.7, 0.7);
	// Hind leg, on its own layer so it reads as sitting in front of the body.
	limb(leg, 24, 14, 47, 4, 3, 1.2);
	limb(leg, 47, 4, 41, 25, 0.8, 0.8);
	limb(leg, 41, 25, 46, 26, 0.7, 0.7);

	const touchesLeg = (x: number, y: number) => {
		for (let dy = -1; dy <= 1; dy++) {
			for (let dx = -1; dx <= 1; dx++) {
				const xx = x + dx;
				const yy = y + dy;
				if (xx >= 0 && yy >= 0 && xx < cols && yy < rows && leg[yy * cols + xx]) return true;
			}
		}
		return false;
	};

	const mask = new Uint8Array(cols * rows);
	for (let y = 0; y < rows; y++) {
		for (let x = 0; x < cols; x++) {
			const i = y * cols + x;
			// Body cells next to the hind leg are removed, leaving a one-cell outline around it.
			if (leg[i]) mask[i] = LEG;
			else if (body[i] && !touchesLeg(x, y)) mask[i] = BODY;
		}
	}
	mask[13 * cols + 8] = EYE;
	mask[13 * cols + 9] = EYE;
	mask[14 * cols + 8] = EYE;
	return mask;
}

export function createLocust(): Locust {
	const mask = buildLocustMask();
	const count = mask.reduce((n, value) => n + (value ? 1 : 0), 0);
	const grid = new Float32Array(count * 2);
	const kind = new Uint8Array(count);
	const cellAt = new Int32Array(mask.length).fill(-1);

	let next = 0;
	for (const value of [BODY, LEG, EYE]) {
		for (let i = 0; i < mask.length; i++) {
			if (mask[i] !== value) continue;
			grid[next * 2] = i % LOCUST_COLS;
			grid[next * 2 + 1] = Math.floor(i / LOCUST_COLS);
			kind[next] = value;
			cellAt[i] = next++;
		}
	}

	const neighborStart = new Uint32Array(count + 1);
	const neighbors: number[] = [];
	for (let cell = 0; cell < count; cell++) {
		const x = grid[cell * 2];
		const y = grid[cell * 2 + 1];
		for (let dy = -1; dy <= 1; dy++) {
			for (let dx = -1; dx <= 1; dx++) {
				const xx = x + dx;
				const yy = y + dy;
				if ((!dx && !dy) || xx < 0 || yy < 0 || xx >= LOCUST_COLS || yy >= LOCUST_ROWS) continue;
				const neighbor = cellAt[yy * LOCUST_COLS + xx];
				if (neighbor >= 0) neighbors.push(neighbor);
			}
		}
		neighborStart[cell + 1] = neighbors.length;
	}

	return { count, grid, kind, neighborStart, neighbors: Uint32Array.from(neighbors) };
}

export interface LocustLayout {
	/** The locust is not drawn on narrow canvases, where it would sit behind the text. */
	readonly hidden: boolean;
	/** Distance between cell origins, in CSS pixels. Cells are one pixel smaller, leaving a gap. */
	readonly pitch: number;
	/** Top-left corner of the grid, in CSS pixels. */
	readonly originX: number;
	readonly originY: number;
}

/** Places the grid at the right of a canvas of the given CSS size, slightly above centre. */
export function layoutLocust(width: number, height: number): LocustLayout {
	const fit = Math.floor(Math.min((width * 0.6) / LOCUST_COLS, (height * 0.62) / LOCUST_ROWS));
	const pitch = Math.max(6, Math.min(18, fit));
	return {
		hidden: width < 960,
		pitch,
		originX: width - Math.max(24, width * 0.04) - LOCUST_COLS * pitch,
		originY: height * 0.44 - (LOCUST_ROWS * pitch) / 2
	};
}

/** A random starting generation: one byte per cell, 1 if alive. */
export function seedLife(locust: Locust, random: () => number = Math.random): Uint8Array {
	return Uint8Array.from({ length: locust.count }, () => (random() < LIFE_INITIAL_ALIVE ? 1 : 0));
}

/**
 * Advances `alive` by one generation, in place. Standard Life rules, plus a small
 * chance of spontaneous birth and a reseed when the population nearly dies out, so
 * the silhouette never goes dark.
 */
export function stepLife(
	locust: Locust,
	alive: Uint8Array,
	random: () => number = Math.random
): void {
	const { count, neighborStart, neighbors } = locust;
	const next = new Uint8Array(count);
	let population = 0;

	for (let cell = 0; cell < count; cell++) {
		let around = 0;
		for (let i = neighborStart[cell]; i < neighborStart[cell + 1]; i++)
			around += alive[neighbors[i]];
		const lives = alive[cell]
			? around === 2 || around === 3
			: around === 3 || random() < LIFE_SPONTANEOUS_BIRTH;
		if (lives) {
			next[cell] = 1;
			population++;
		}
	}
	alive.set(next);

	if (population < count * LIFE_RESEED_BELOW) {
		for (let cell = 0; cell < count; cell++) {
			if (random() < LIFE_RESEED_CHANCE) alive[cell] = 1;
		}
	}
}
