import { GUIDE_PATH } from '#lib/onboarding/guide.ts';

export const HOME_PATH = '/';

/** Header links after the brand, which itself links home. */
export const NAV_LINKS = [{ label: 'start', href: GUIDE_PATH }] as const;
