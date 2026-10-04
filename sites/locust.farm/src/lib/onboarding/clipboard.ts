export type CopyResult = 'copied' | 'failed';

/** Minimal clipboard surface, so tests can pass a fake. */
export interface ClipboardLike {
	writeText(text: string): Promise<void>;
}

export const COPY_MESSAGES = {
	copied: 'Copied. Paste it into your agent.',
	failed: 'Copy did not work. The prompt is selected: press Ctrl+C or Cmd+C to copy it.'
} as const;

/**
 * Writes `text` to the supplied clipboard and reports whether the write actually
 * succeeded. Never throws: a missing clipboard, a synchronous throw, or a
 * rejected promise all resolve to 'failed'.
 */
export async function copyText(
	text: string,
	clipboard: ClipboardLike | undefined
): Promise<CopyResult> {
	if (clipboard == null || typeof clipboard.writeText !== 'function') {
		return 'failed';
	}
	try {
		await clipboard.writeText(text);
	} catch {
		return 'failed';
	}
	return 'copied';
}
