import { GUIDE_PATH } from '#lib/onboarding/guide.ts';

export const HOME_PATH = '/';

export const REPOSITORY_URL = 'https://github.com/andsav/locust.farm';

/** Header links after the brand, which itself links home. */
export const NAV_LINKS = [
	{ label: 'start', href: GUIDE_PATH },
	{ label: 'github', href: REPOSITORY_URL }
] as const;
