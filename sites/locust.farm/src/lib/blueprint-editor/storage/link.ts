// Share links. The blueprint travels after "#" in the address, compressed. The
// part after "#" is not sent to any server, but it stays in browser history
// and anywhere the link is pasted.

const PREFIX = 'b1.';
/** Links longer than this are refused when opened. */
export const MAX_FRAGMENT = 64 * 1024;
/** A link may not expand to more than this. */
const MAX_EXPANDED = 1024 * 1024;
/** Above this length a link is unwieldy; the page suggests a download instead. */
export const LONG_LINK = 8 * 1024;

async function pipe(
	bytes: Uint8Array,
	stream: CompressionStream | DecompressionStream,
	limit: number
) {
	const reader = new Blob([bytes as BlobPart]).stream().pipeThrough(stream).getReader();
	const chunks: Uint8Array[] = [];
	let total = 0;
	for (;;) {
		const { done, value } = await reader.read();
		if (done) break;
		total += value.length;
		if (total > limit) {
			await reader.cancel();
			throw new Error('too large');
		}
		chunks.push(value);
	}
	const out = new Uint8Array(total);
	let offset = 0;
	for (const chunk of chunks) {
		out.set(chunk, offset);
		offset += chunk.length;
	}
	return out;
}

function toBase64Url(bytes: Uint8Array): string {
	let binary = '';
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
}

function fromBase64Url(text: string): Uint8Array {
	const base64 = text.replaceAll('-', '+').replaceAll('_', '/');
	const binary = atob(base64 + '='.repeat((4 - (base64.length % 4)) % 4));
	return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}

/** The fragment for some text, without the leading "#". */
export async function encodeLink(text: string): Promise<string> {
	const compressed = await pipe(
		new TextEncoder().encode(text),
		new CompressionStream('deflate-raw'),
		Infinity
	);
	return PREFIX + toBase64Url(compressed);
}

export type DecodedLink = { ok: true; text: string } | { ok: false; message: string } | null;

/** Reads a fragment. Returns null when the fragment is not a blueprint link. */
export async function decodeLink(fragment: string): Promise<DecodedLink> {
	const value = fragment.startsWith('#') ? fragment.slice(1) : fragment;
	if (!value.startsWith(PREFIX)) return null;
	if (value.length > MAX_FRAGMENT)
		return { ok: false, message: 'This link is too large to open here.' };
	try {
		const bytes = await pipe(
			fromBase64Url(value.slice(PREFIX.length)),
			new DecompressionStream('deflate-raw'),
			MAX_EXPANDED
		);
		return { ok: true, text: new TextDecoder('utf-8', { fatal: true }).decode(bytes) };
	} catch {
		return { ok: false, message: 'This link is damaged or too large. Ask for a new one.' };
	}
}
