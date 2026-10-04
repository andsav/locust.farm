// Opens text someone pastes or chooses as a file: raw blueprint JSON, a prompt
// copied from this page, or an agent's reply that repeats the blueprint block.
// Pasted text is read leniently, because terminals and chat windows change
// indentation and line endings; the blueprint is then checked strictly.

import { decodeBlueprint } from '../contract/decode.ts';
import { inspect } from '../contract/inspect.ts';
import { parseJson } from '../contract/json.ts';
import type { Diagnostic } from '../contract/types.ts';
import { readPresentation, type EditorDocument } from '../model/document.ts';
import { BLUEPRINT_BEGIN, BLUEPRINT_END, LAYOUT_BEGIN, LAYOUT_END } from './prompt.ts';

export type Opened =
	| { ok: true; document: EditorDocument; note: string | null }
	| { ok: false; message: string; diagnostics: Diagnostic[] };

/** Lines between a BEGIN line and its END line, with shared indentation removed. */
function between(lines: string[], begin: string, end: string): string | null {
	const start = lines.findIndex((line) => line.trimStart().startsWith(begin));
	if (start === -1) return null;
	const stop = lines.findIndex((line, index) => index > start && line.trimStart().startsWith(end));
	if (stop === -1) return null;
	const body = lines.slice(start + 1, stop);
	const indents = body
		.filter((line) => line.trim() !== '')
		.map((line) => /^[ \t]*/.exec(line)![0].length);
	const shared = indents.length === 0 ? 0 : Math.min(...indents);
	return body.map((line) => line.slice(shared).trimEnd()).join('\n');
}

export function openText(text: string): Opened {
	const lines = text.replace(/\r\n?/g, '\n').split('\n');
	const block = between(lines, BLUEPRINT_BEGIN, BLUEPRINT_END);
	const layout = between(lines, LAYOUT_BEGIN, LAYOUT_END);
	const source = block ?? text.trim();
	if (block === null && !source.startsWith('{')) {
		return {
			ok: false,
			message:
				'No blueprint found. Paste the blueprint JSON, or the whole reply that contains the lines BEGIN LOCUST BLUEPRINT and END LOCUST BLUEPRINT.',
			diagnostics: []
		};
	}
	const inspection = inspect(source);
	const loadProblem = inspection.diagnostics.find(
		(d) => d.phase === 'load' || d.code === 'invalid_structure'
	);
	if (loadProblem) {
		return {
			ok: false,
			message:
				loadProblem.code === 'unsupported_version'
					? 'This blueprint is for a different blueprint format. It cannot be opened here and is not converted.'
					: `This blueprint cannot be opened: ${loadProblem.message}`,
			diagnostics: [loadProblem]
		};
	}
	const parsed = parseJson(source);
	if (!parsed.ok) return { ok: false, message: parsed.error.message, diagnostics: [] };
	const decoded = decodeBlueprint(parsed.value);
	if (!decoded.ok) return { ok: false, message: decoded.error.message, diagnostics: [] };
	let presentationValue: unknown = null;
	if (layout !== null) {
		try {
			presentationValue = JSON.parse(layout);
		} catch {
			presentationValue = null;
		}
	}
	const read = readPresentation(presentationValue);
	return {
		ok: true,
		document: {
			name: read.name ?? '',
			blueprint: decoded.blueprint,
			layout: read.layout,
			way: read.way
		},
		note:
			layout !== null && presentationValue === null
				? 'The layout could not be read, so stages are placed automatically.'
				: null
	};
}
