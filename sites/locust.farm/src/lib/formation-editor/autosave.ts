// Capture content and identity together. Ordering is per record, even when
// saves for other records complete between two versions of the same document.

import { blocks } from './prompt/prompt.ts';
import { save, type KeyValueStore, type SavedRecord } from './storage/saved.ts';
import type { EditorDocument } from './model/document.ts';

export interface SaveStatus {
	ok: boolean;
	text: string;
}

export interface AutosaveSnapshot {
	document: EditorDocument;
	recordId: string;
	origin: 'new' | 'link' | 'import';
}

export type TimerHandle = ReturnType<typeof setTimeout>;

export interface AutosaveTimers {
	set: (callback: () => void, ms: number) => TimerHandle;
	clear: (handle: TimerHandle | undefined) => void;
}

export interface AutosaveOptions {
	debounceMs: number;
	timers?: AutosaveTimers;
	hash?: (document: EditorDocument) => Promise<string>;
	persist?: (store: KeyValueStore, record: SavedRecord) => boolean;
	onStatus?: (status: SaveStatus) => void;
	currentRecordId?: () => string;
}

const NOT_STORED: SaveStatus = {
	ok: false,
	text: "Not saved: this browser isn't keeping data for this page. Download or copy a link to keep your work."
};
const REFUSED: SaveStatus = {
	ok: false,
	text: 'Not saved: this browser refused to store it. Download or copy a link to keep your work.'
};
const SAVED: SaveStatus = { ok: true, text: 'Saved in this browser.' };

interface Pending {
	store: KeyValueStore | null;
	snapshot: AutosaveSnapshot;
	generation: number;
}

/** A debounced, ordered autosave that captures record identity with each edit. */
export class Autosaver {
	private readonly debounceMs: number;
	private readonly timers: AutosaveTimers;
	private readonly hash: (document: EditorDocument) => Promise<string>;
	private readonly persist: (store: KeyValueStore, record: SavedRecord) => boolean;
	private readonly onStatus: (status: SaveStatus) => void;
	private readonly currentRecordId: () => string;
	private pending: Pending | null = null;
	private readonly latest = new Map<string, number>();
	private nextGeneration = 1;
	private timer: TimerHandle | undefined;
	private destroyed = false;

	constructor(options: AutosaveOptions) {
		this.debounceMs = options.debounceMs;
		this.timers = options.timers ?? {
			set: (cb, ms) => setTimeout(cb, ms),
			clear: (h) => {
				if (h !== undefined) clearTimeout(h);
			}
		};
		this.hash = options.hash ?? (async (document) => (await blocks(document)).text);
		this.persist = options.persist ?? save;
		this.onStatus = options.onStatus ?? (() => {});
		this.currentRecordId = options.currentRecordId ?? (() => '');
	}

	/** Schedule a debounced save for the given snapshot. Replaces any pending save. */
	schedule(store: KeyValueStore | null, snapshot: AutosaveSnapshot): void {
		if (this.destroyed) return;
		const generation = this.nextGeneration++;
		if (this.pending && this.pending.snapshot.recordId !== snapshot.recordId) void this.flush();
		this.latest.set(snapshot.recordId, generation);
		this.pending = {
			store,
			snapshot: structuredClone(snapshot),
			generation
		};
		this.timers.clear(this.timer);
		this.timer = this.timers.set(() => {
			if (this.pending?.generation === generation) void this.flush();
		}, this.debounceMs);
	}

	/** Flush the pending snapshot immediately, before switching records. */
	flush(): Promise<void> {
		this.timers.clear(this.timer);
		this.timer = undefined;
		const pending = this.pending;
		this.pending = null;
		return pending ? this.fire(pending) : Promise.resolve();
	}

	/** A deleted record must not be recreated by pending or in-flight work. */
	discard(recordId: string): void {
		this.latest.delete(recordId);
		if (this.pending?.snapshot.recordId === recordId) {
			this.timers.clear(this.timer);
			this.timer = undefined;
			this.pending = null;
		}
	}

	/** Flush the last edit without updating a destroyed component's status. */
	teardown(): void {
		this.destroyed = true;
		void this.flush();
	}

	private async fire({ store, snapshot, generation }: Pending): Promise<void> {
		const current = () => this.latest.get(snapshot.recordId) === generation;
		let status = NOT_STORED;
		try {
			if (store) {
				const text = await this.hash(snapshot.document);
				if (!current()) return;
				const ok = this.persist(store, {
					id: snapshot.recordId,
					name: snapshot.document.name,
					origin: snapshot.origin,
					saved: new Date().toISOString(),
					data: text
				});
				status = ok ? SAVED : REFUSED;
			}
		} catch {
			status = {
				ok: false,
				text: 'Not saved: this browser could not prepare or store the formation. Download or copy a link to keep your work.'
			};
		}
		if (current() && !this.destroyed && this.currentRecordId() === snapshot.recordId)
			this.onStatus(status);
	}
}
