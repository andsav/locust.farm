// Undo and redo over whole documents. One user action is one entry.

import type { EditorDocument } from './document.ts';

const LIMIT = 100;

export interface History {
	past: EditorDocument[];
	present: EditorDocument;
	future: EditorDocument[];
}

export function startHistory(document: EditorDocument): History {
	return { past: [], present: document, future: [] };
}

/** Records a new state. A change that returns the same object is not recorded. */
export function record(history: History, next: EditorDocument): History {
	if (next === history.present) return history;
	return {
		past: [...history.past, history.present].slice(-LIMIT),
		present: next,
		future: []
	};
}

export function undo(history: History): History {
	if (history.past.length === 0) return history;
	return {
		past: history.past.slice(0, -1),
		present: history.past[history.past.length - 1],
		future: [history.present, ...history.future]
	};
}

export function redo(history: History): History {
	if (history.future.length === 0) return history;
	return {
		past: [...history.past, history.present],
		present: history.future[0],
		future: history.future.slice(1)
	};
}
