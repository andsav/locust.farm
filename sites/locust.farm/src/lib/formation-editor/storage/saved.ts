// Formations kept in this browser. Each is its own record, so opening a link,
// importing or starting over never replaces earlier work. Storage can be
// missing or full; every access is guarded and reports what happened.

export interface SavedRecord {
	id: string;
	name: string;
	origin: 'new' | 'link' | 'import';
	saved: string;
	/** The prompt's data blocks: formation and layout. */
	data: string;
}

export type SavedSummary = Omit<SavedRecord, 'data'>;

export interface KeyValueStore {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
	removeItem(key: string): void;
}

const PREFIX = 'locust:formation-editor:2:';
const INDEX = `${PREFIX}index`;

function readIndex(store: KeyValueStore): SavedSummary[] {
	try {
		const value = JSON.parse(store.getItem(INDEX) ?? '[]');
		return Array.isArray(value) ? value : [];
	} catch {
		return [];
	}
}

export function listSaved(store: KeyValueStore): SavedSummary[] {
	return readIndex(store).sort((a, b) => b.saved.localeCompare(a.saved));
}

export function loadSaved(store: KeyValueStore, id: string): SavedRecord | null {
	try {
		const value = JSON.parse(store.getItem(`${PREFIX}${id}`) ?? 'null');
		return value && typeof value.data === 'string' ? (value as SavedRecord) : null;
	} catch {
		return null;
	}
}

/** Writes a record. Returns false when the browser refused to store it. */
export function save(store: KeyValueStore, record: SavedRecord): boolean {
	try {
		store.setItem(`${PREFIX}${record.id}`, JSON.stringify(record));
		const index = readIndex(store).filter((item) => item.id !== record.id);
		index.push({
			id: record.id,
			name: record.name,
			origin: record.origin,
			saved: record.saved
		});
		store.setItem(INDEX, JSON.stringify(index));
		return true;
	} catch {
		return false;
	}
}

export function removeSaved(store: KeyValueStore, id: string): boolean {
	try {
		store.removeItem(`${PREFIX}${id}`);
		store.setItem(INDEX, JSON.stringify(readIndex(store).filter((item) => item.id !== id)));
		return true;
	} catch {
		return false;
	}
}

export function newRecordId(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(6));
	return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

/** The browser's local storage, or null when this page may not use it. */
export function browserStore(): KeyValueStore | null {
	try {
		const store = globalThis.localStorage;
		const probe = `${PREFIX}probe`;
		store.setItem(probe, '1');
		store.removeItem(probe);
		return store;
	} catch {
		return null;
	}
}
