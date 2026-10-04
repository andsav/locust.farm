import { GUIDE_PATH } from '#lib/onboarding/guide.ts';

export const HOME_PATH = '/';
export const DOCS_PATH = '/docs';
export const BLUEPRINTS_PATH = '/blueprints';

/** Header links after the brand, which itself links home. */
export const NAV_LINKS = [
	{ label: 'start', href: GUIDE_PATH },
	{ label: 'blueprints', href: BLUEPRINTS_PATH },
	{ label: 'docs', href: DOCS_PATH }
] as const;
