import assert from 'node:assert/strict';
import { test } from 'node:test';
import { watchFarm, type FarmState } from './client.ts';

class Stream extends EventTarget {
	onopen: (() => void) | null = null;
	onerror: (() => void) | null = null;
	closed = false;
	close() {
		this.closed = true;
	}
	state(state: FarmState) {
		this.dispatchEvent(new MessageEvent('state', { data: JSON.stringify(state) }));
	}
}
const state = (version: number): FarmState => ({
	farm_id: 'example',
	stream_version: version,
	service_time_ms: 0
});

test('an older GET cannot overwrite a newer stream and invalidation clears existing snapshot', async () => {
	const stream = new Stream();
	const seen: FarmState[] = [];
	let answer!: (value: Response) => void;
	const stop = watchFarm(
		'example',
		(next) => seen.push(next),
		() => {},
		{
			fetch: (() =>
				new Promise<Response>((resolve) => {
					answer = resolve;
				})) as typeof fetch,
			source: () => stream as unknown as EventSource
		}
	);
	stream.state(state(4));
	answer(new Response(JSON.stringify(state(3))));
	await new Promise((resolve) => setTimeout(resolve, 0));
	assert.deepEqual(
		seen.map((next) => next.stream_version),
		[4]
	);
	stream.state({ ...state(5), status: 'unavailable' });
	assert.equal(seen.at(-1)?.status, 'unavailable');
	assert.equal(seen.at(-1)?.snapshot, undefined);
	stream.state(state(4));
	assert.equal(seen.length, 2);
	stop();
	assert.equal(stream.closed, true);
	stream.state(state(6));
	assert.equal(seen.length, 2);
});

test('reconnect receives a replacement full state and disconnect is exposed', () => {
	const stream = new Stream();
	const connections: string[] = [];
	const seen: FarmState[] = [];
	const stop = watchFarm(
		'example',
		(next) => seen.push(next),
		(next) => connections.push(next),
		{
			fetch: (() => new Promise(() => {})) as typeof fetch,
			source: () => stream as unknown as EventSource
		}
	);
	stream.state(state(2));
	stream.onerror?.();
	stream.state(state(8));
	assert.deepEqual(
		seen.map((next) => next.stream_version),
		[2, 8]
	);
	assert.deepEqual(connections, ['connecting', 'connected', 'disconnected', 'connected']);
	stop();
});
