// A strict JSON reader, matching Locust's loader
// (crates/locust-core/src/organization/strict_json.rs): duplicate keys are
// refused, numbers keep their exact text, and nesting is limited as in
// serde_json. JSON.parse cannot be used because it keeps the last of two
// duplicate keys silently.

import { escapePointer } from './text.ts';

export interface JsonNumber {
	type: 'number';
	text: string;
}

export interface JsonObject {
	type: 'object';
	/** Entries in document order. Keys are unique. */
	entries: [string, JsonValue][];
}

export type JsonValue = null | boolean | string | JsonNumber | JsonObject | JsonValue[];

export interface JsonError {
	code: 'invalid_json' | 'duplicate_key';
	/** JSON Pointer of the duplicated key, or '' for other errors. */
	path: string;
	message: string;
}

export type JsonResult = { ok: true; value: JsonValue } | { ok: false; error: JsonError };

// serde_json refuses documents nested deeper than this.
const MAX_DEPTH = 128;

class Failure extends Error {
	readonly code: JsonError['code'];
	readonly path: string;
	constructor(code: JsonError['code'], path: string, message: string) {
		super(message);
		this.code = code;
		this.path = path;
	}
}

export function parseJson(source: string): JsonResult {
	let position = 0;

	const where = () => {
		const before = source.slice(0, position);
		const line = before.split('\n').length;
		const column = position - before.lastIndexOf('\n');
		return `line ${line}, column ${column}`;
	};
	const fail = (message: string): never => {
		throw new Failure('invalid_json', '', `${message} at ${where()}.`);
	};
	const skipSpace = () => {
		while (position < source.length) {
			const char = source[position];
			if (char !== ' ' && char !== '\t' && char !== '\n' && char !== '\r') break;
			position += 1;
		}
	};
	const expectWord = (word: string) => {
		if (source.startsWith(word, position)) position += word.length;
		else fail('Unexpected text');
	};

	const readString = (): string => {
		position += 1; // opening quote
		let out = '';
		for (;;) {
			if (position >= source.length) fail('The text ends inside a string');
			const char = source[position];
			const code = char.charCodeAt(0);
			if (char === '"') {
				position += 1;
				return out;
			}
			if (code < 0x20) fail('A control character must be escaped inside a string');
			if (char !== '\\') {
				out += char;
				position += 1;
				continue;
			}
			const escape = source[position + 1];
			position += 2;
			switch (escape) {
				case '"':
					out += '"';
					break;
				case '\\':
					out += '\\';
					break;
				case '/':
					out += '/';
					break;
				case 'b':
					out += '\b';
					break;
				case 'f':
					out += '\f';
					break;
				case 'n':
					out += '\n';
					break;
				case 'r':
					out += '\r';
					break;
				case 't':
					out += '\t';
					break;
				case 'u': {
					const unit = readHex();
					if (unit >= 0xd800 && unit <= 0xdbff) {
						if (source[position] !== '\\' || source[position + 1] !== 'u') {
							fail('A lone surrogate is not allowed');
						}
						position += 2;
						const low = readHex();
						if (low < 0xdc00 || low > 0xdfff) fail('A lone surrogate is not allowed');
						out += String.fromCharCode(unit, low);
					} else if (unit >= 0xdc00 && unit <= 0xdfff) {
						fail('A lone surrogate is not allowed');
					} else {
						out += String.fromCharCode(unit);
					}
					break;
				}
				default:
					fail('Unknown escape in a string');
			}
		}
	};
	const readHex = (): number => {
		const digits = source.slice(position, position + 4);
		if (!/^[0-9a-fA-F]{4}$/.test(digits)) fail('Expected four hex digits after \\u');
		position += 4;
		return parseInt(digits, 16);
	};

	const readNumber = (): JsonNumber => {
		const match = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/.exec(
			source.slice(position)
		);
		if (!match) fail('Invalid number');
		const text = match![0];
		position += text.length;
		const next = source[position];
		if (next !== undefined && /[0-9.eE+-]/.test(next)) fail('Invalid number');
		if (!Number.isFinite(Number(text))) fail('The number is out of range');
		return { type: 'number', text };
	};

	const readValue = (path: string, depth: number): JsonValue => {
		skipSpace();
		if (position >= source.length) fail('The text ends before a value');
		const char = source[position];
		if (char === '{' || char === '[') {
			if (depth >= MAX_DEPTH) fail('The document is nested too deeply');
		}
		if (char === '{') {
			position += 1;
			const entries: [string, JsonValue][] = [];
			const seen = new Set<string>();
			skipSpace();
			if (source[position] === '}') {
				position += 1;
				return { type: 'object', entries };
			}
			for (;;) {
				skipSpace();
				if (source[position] !== '"') fail('Expected a key in quotes');
				const key = readString();
				const childPath = `${path}/${escapePointer(key)}`;
				if (seen.has(key)) {
					throw new Failure('duplicate_key', childPath, `The key "${key}" appears twice.`);
				}
				seen.add(key);
				skipSpace();
				if (source[position] !== ':') fail('Expected a colon');
				position += 1;
				entries.push([key, readValue(childPath, depth + 1)]);
				skipSpace();
				if (source[position] === ',') {
					position += 1;
					continue;
				}
				if (source[position] === '}') {
					position += 1;
					return { type: 'object', entries };
				}
				fail('Expected a comma or a closing brace');
			}
		}
		if (char === '[') {
			position += 1;
			const items: JsonValue[] = [];
			skipSpace();
			if (source[position] === ']') {
				position += 1;
				return items;
			}
			for (;;) {
				items.push(readValue(`${path}/${items.length}`, depth + 1));
				skipSpace();
				if (source[position] === ',') {
					position += 1;
					continue;
				}
				if (source[position] === ']') {
					position += 1;
					return items;
				}
				fail('Expected a comma or a closing bracket');
			}
		}
		if (char === '"') return readString();
		if (char === 't') {
			expectWord('true');
			return true;
		}
		if (char === 'f') {
			expectWord('false');
			return false;
		}
		if (char === 'n') {
			expectWord('null');
			return null;
		}
		if (char === '-' || (char >= '0' && char <= '9')) return readNumber();
		return fail('Unexpected character');
	};

	try {
		const value = readValue('', 0);
		skipSpace();
		if (position < source.length) fail('Unexpected text after the document');
		return { ok: true, value };
	} catch (error) {
		if (error instanceof Failure) {
			return { ok: false, error: { code: error.code, path: error.path, message: error.message } };
		}
		throw error;
	}
}

/** Looks up a key in a parsed object. */
export function field(object: JsonObject, key: string): JsonValue | undefined {
	for (const [name, value] of object.entries) if (name === key) return value;
	return undefined;
}

export function isObject(value: JsonValue | undefined): value is JsonObject {
	return (
		typeof value === 'object' && value !== null && !Array.isArray(value) && value.type === 'object'
	);
}

export function isNumber(value: JsonValue | undefined): value is JsonNumber {
	return (
		typeof value === 'object' && value !== null && !Array.isArray(value) && value.type === 'number'
	);
}
