import type { FarmSnapshot } from './types.ts';

export interface FarmState {
	/** Client receipt used only to advance the service clock between envelopes. */
	local_received_at_ms?: number;
	farm_id: string;
	stream_version: number;
	service_time_ms: number;
	received_at_ms?: number | null;
	snapshot?: FarmSnapshot | null;
	status?: 'available' | 'unavailable';
	visibility?: 'link' | 'listed' | null;
}
export interface FarmListing {
	farms: FarmState[];
	next_cursor: string | null;
}
export const serviceNow = (view: FarmState | null, now: number): number =>
	view?.local_received_at_ms == null
		? now
		: view.service_time_ms + Math.max(0, now - view.local_received_at_ms);
export type Connection = 'connecting' | 'connected' | 'disconnected';

export function newer(current: FarmState | null, incoming: FarmState): FarmState {
	return !current || incoming.stream_version > current.stream_version ? incoming : current;
}

/** Subscribe before GET. Full stream states and version comparison close the fetch race. */
export function watchFarm(
	id: string,
	onState: (state: FarmState) => void,
	onConnection: (connection: Connection) => void,
	dependencies: { fetch: typeof fetch; source: (url: string) => EventSource } = {
		fetch: globalThis.fetch.bind(globalThis),
		source: (url) => new EventSource(url)
	}
): () => void {
	const endpoint = `/api/farms/${encodeURIComponent(id)}`;
	let current: FarmState | null = null;
	let disposed = false;
	const controller = new AbortController();
	const accept = (state: FarmState) => {
		if (disposed || state.farm_id !== id) return;
		const next = newer(current, state);
		if (next !== current) {
			current = { ...next, local_received_at_ms: Date.now() };
			onState(current);
		}
	};
	onConnection('connecting');
	const stream = dependencies.source(`${endpoint}/events`);
	stream.addEventListener('state', (event) => {
		if (disposed) return;
		try {
			accept(JSON.parse((event as MessageEvent).data));
			onConnection('connected');
		} catch {
			onConnection('disconnected');
		}
	});
	stream.onopen = () => {
		if (!disposed) onConnection('connected');
	};
	stream.onerror = () => {
		if (!disposed) onConnection('disconnected');
	};
	void dependencies
		.fetch(endpoint, { cache: 'no-store', signal: controller.signal })
		.then(async (response) => {
			if (response.ok || response.status === 404 || response.status === 410)
				accept(await response.json());
			else if (!current) onConnection('disconnected');
		})
		.catch(() => {
			if (!disposed && !current) onConnection('disconnected');
		});
	return () => {
		disposed = true;
		controller.abort();
		stream.close();
	};
}
