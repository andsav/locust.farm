import { GUIDE_PATH } from '#lib/onboarding/guide.ts';

export const HOME_PATH = '/';
export const DOCS_PATH = '/docs';
export const FORMATIONS_PATH = '/formations';
export const FARMS_PATH = '/farms';

/** Header links after the brand, which itself links home. */
export const NAV_LINKS = [
	{ label: 'Start', href: GUIDE_PATH },
	{ label: 'Formations', href: FORMATIONS_PATH },
	{ label: 'Farms', href: FARMS_PATH },
	{ label: 'Docs', href: DOCS_PATH }
] as const;
