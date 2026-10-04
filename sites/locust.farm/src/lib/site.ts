import { GUIDE_PATH } from '#lib/onboarding/guide.ts';

export const HOME_PATH = '/';
export const DOCS_PATH = '/docs';
export const FORMATIONS_PATH = '/formations';

/** Header links after the brand, which itself links home. */
export const NAV_LINKS = [
	{ label: 'start', href: GUIDE_PATH },
	{ label: 'formations', href: FORMATIONS_PATH },
	{ label: 'docs', href: DOCS_PATH }
] as const;
