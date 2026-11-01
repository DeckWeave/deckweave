import type { Page, Profile } from './types';

// Folders have names; numeric defaults on root pages are their visible ordinal.
export function pageLabel(profile: Profile, page: Page): string {
  if (page.parent !== null || !/^\d+$/.test(page.name)) return page.name;
  return String(
    profile.pages.filter((p) => p.parent === null).findIndex((p) => p.id === page.id) + 1,
  );
}
