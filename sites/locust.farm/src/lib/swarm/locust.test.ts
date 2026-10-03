import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
	BODY,
	EYE,
	LEG,
	LOCUST_COLS,
	LOCUST_ROWS,
	buildLocustMask,
	createLocust,
	seedLife,
	stepLife
} from './locust.ts';

// The silhouette from the design mock: '#' body, '+' hind leg, 'o' eye.
const SILHOUETTE = [
	'                                                          ',
	' #                                                        ',
	'  #    #                                                  ',
	'   #   ##                                    +++          ',
	'    #   #                                 ++++++          ',
	'    ##   #                             ++++++++           ',
	'     #   #                          +++++++++++           ',
	'      #  ##                      ++++++++++  ++           ',
	'      ##  #   ####             ++++++++++    ++           ',
	'       #  #  ###########    +++++++++++      +            ',
	'      ###############    ++++++++++++       ++            ',
	'      ##############  +++++++++++++   ##### ++ ########   ',
	'     ############### ++++++++++++   ######  +  #########  ',
	'     ###oo########## ++++++++++   ######## ++ #########   ',
	'     ###o########### +++++++++  ########## ++ ##########  ',
	'     ############### +++++++   ########### ++ #########   ',
	'     ###############  ++++   ############  +  ##          ',
	'     ################      ##########     ++              ',
	'      ####### ############                ++              ',
	'     ########     ##                      +               ',
	'     ####  #       #                     ++               ',
	'      ##  ##       ##                    ++               ',
	'          ##       ##                    ++               ',
	'          #         #                    +                ',
	'         ##         ##                  ++                ',
	'       ####         ####                ++++++            ',
	'       #               #                     +            ',
	'                                                          ',
	'                                                          '
];

const never = () => 1;
const always = () => 0;

test('the mask matches the design silhouette', () => {
	const mask = buildLocustMask();
	const drawn = Array.from({ length: LOCUST_ROWS }, (_, y) =>
		Array.from({ length: LOCUST_COLS }, (_, x) => ' #+o'[mask[y * LOCUST_COLS + x]]).join('')
	);
	assert.deepEqual(drawn, SILHOUETTE);
});

test('cells are ordered body, leg, eye and keep their grid position', () => {
	const locust = createLocust();
	assert.equal(locust.count, 490);
	assert.deepEqual(Array.from(locust.kind), [
		...Array(323).fill(BODY),
		...Array(164).fill(LEG),
		...Array(3).fill(EYE)
	]);
	for (let cell = 0; cell < locust.count; cell++) {
		const x = locust.grid[cell * 2];
		const y = locust.grid[cell * 2 + 1];
		assert.equal(' #+o'[locust.kind[cell]], SILHOUETTE[y][x]);
	}
});

test('neighbours are the adjacent cells of the silhouette, in both directions', () => {
	const locust = createLocust();
	for (let cell = 0; cell < locust.count; cell++) {
		for (let i = locust.neighborStart[cell]; i < locust.neighborStart[cell + 1]; i++) {
			const other = locust.neighbors[i];
			assert.notEqual(other, cell);
			assert.ok(Math.abs(locust.grid[other * 2] - locust.grid[cell * 2]) <= 1);
			assert.ok(Math.abs(locust.grid[other * 2 + 1] - locust.grid[cell * 2 + 1]) <= 1);
			const back = locust.neighbors.subarray(
				locust.neighborStart[other],
				locust.neighborStart[other + 1]
			);
			assert.ok(back.includes(cell));
		}
	}
	// The eye cell at (8, 13) sits inside the head, so all eight neighbours exist.
	const eye = locust.count - 3;
	assert.equal(locust.neighborStart[eye + 1] - locust.neighborStart[eye], 8);
});

test('life follows the standard rules inside the silhouette', () => {
	const locust = createLocust();
	const eye = locust.count - 3;
	const around = Array.from(
		locust.neighbors.subarray(locust.neighborStart[eye], locust.neighborStart[eye + 1])
	);
	const generation = (...cells: number[]) => {
		const alive = new Uint8Array(locust.count);
		// Enough distant cells stay alive to keep the population above the reseed threshold.
		for (let cell = 60; cell < 300; cell++) alive[cell] = 1;
		for (const cell of [eye, ...around]) alive[cell] = 0;
		for (const cell of cells) alive[cell] = 1;
		stepLife(locust, alive, never);
		return alive;
	};

	assert.equal(generation(...around.slice(0, 3))[eye], 1, 'born with three neighbours');
	assert.equal(generation(...around.slice(0, 2))[eye], 0, 'not born with two');
	assert.equal(generation(eye, ...around.slice(0, 2))[eye], 1, 'survives with two');
	assert.equal(generation(eye, ...around.slice(0, 3))[eye], 1, 'survives with three');
	assert.equal(generation(eye, ...around.slice(0, 1))[eye], 0, 'dies of loneliness');
	assert.equal(generation(eye, ...around.slice(0, 4))[eye], 0, 'dies of overcrowding');
});

test('life reseeds when the population nearly dies out', () => {
	const locust = createLocust();
	const alive = new Uint8Array(locust.count);
	let calls = 0;
	// No spontaneous births during the generation, then every reseed roll succeeds.
	stepLife(locust, alive, () => (calls++ < locust.count ? 1 : 0));
	assert.equal(
		alive.reduce((n, value) => n + value, 0),
		locust.count
	);
});

test('seeding uses the supplied random source', () => {
	const locust = createLocust();
	assert.ok(seedLife(locust, always).every((value) => value === 1));
	assert.ok(seedLife(locust, never).every((value) => value === 0));
});
