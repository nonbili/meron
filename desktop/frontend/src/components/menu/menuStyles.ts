/**
 * Shared class strings for popover / context-menu item buttons. Keep typography
 * and layout (13px, normal weight, padding, hover) defined here only — individual
 * menus compose these and append their own extras (disabled states, etc.).
 */
const menuItemBase =
  // leading-normal, not leading-none: labels that truncate clip their own
  // descenders when the line box is only as tall as the font size. The row
  // height is pinned by h-8 + items-center, so the taller line box costs nothing.
  'flex h-8 w-full items-center gap-2 whitespace-nowrap rounded-lg px-2 text-left text-[0.8125rem] font-normal leading-normal cursor-pointer transition-colors'

/** Standard menu item. */
export const menuItemClass = `${menuItemBase} text-primary hover:bg-hover`

/** Destructive menu item (delete / trash). */
export const menuItemDangerClass = `${menuItemBase} text-rose-600 dark:text-rose-400 hover:bg-rose-50 dark:hover:bg-rose-950/25`

/** Base layout/typography only — use when an item needs custom color/active states. */
export { menuItemBase }
