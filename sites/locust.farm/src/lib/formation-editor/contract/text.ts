// Small text helpers that match how Locust's Rust code formats and orders text.

/** Escapes one JSON Pointer segment (RFC 6901), as Locust's `escape` does. */
export function escapePointer(segment: string): string {
	return segment.replaceAll('~', '~0').replaceAll('/', '~1');
}

/**
 * Orders strings by Unicode code point, which equals Rust's `String` ordering
 * (UTF-8 bytes). JavaScript's default comparison uses UTF-16 code units and
 * puts characters above U+FFFF before U+E000 to U+FFFF.
 */
export function compareCodePoints(a: string, b: string): number {
	const left = Array.from(a);
	const right = Array.from(b);
	const length = Math.min(left.length, right.length);
	for (let index = 0; index < length; index += 1) {
		const difference = left[index].codePointAt(0)! - right[index].codePointAt(0)!;
		if (difference !== 0) return difference;
	}
	return left.length - right.length;
}

/** Keys of a record in Rust `BTreeMap` order. */
export function sortedKeys(record: Record<string, unknown>): string[] {
	return Object.keys(record).sort(compareCodePoints);
}

const NOT_PRINTABLE = /[\p{Cc}\p{Cf}\p{Zl}\p{Zp}\p{Co}\p{Cn}\p{Cs}]/u;

/**
 * Rust's `{:?}` for a string: double quotes, with backslash escapes. Characters
 * Rust does not consider printable become `\u{…}`. This follows Rust for ASCII
 * and common text; rare Unicode classes may differ.
 */
export function rustDebug(text: string): string {
	let out = '"';
	for (const char of text) {
		switch (char) {
			case '"':
				out += '\\"';
				break;
			case '\\':
				out += '\\\\';
				break;
			case '\n':
				out += '\\n';
				break;
			case '\r':
				out += '\\r';
				break;
			case '\t':
				out += '\\t';
				break;
			case '\0':
				out += '\\0';
				break;
			default:
				out += NOT_PRINTABLE.test(char) ? `\\u{${char.codePointAt(0)!.toString(16)}}` : char;
		}
	}
	return `${out}"`;
}
