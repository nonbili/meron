/**
 * Shared class strings for popover / context-menu item buttons. Keep typography
 * and layout (13px, normal weight, padding, hover) defined here only — individual
 * menus compose these and append their own extras (disabled states, etc.).
 */
const menuItemBase =
  'flex h-8 w-full items-center gap-2 whitespace-nowrap rounded-lg px-2 text-left text-[13px] font-normal leading-none cursor-pointer transition-colors'

/** Standard menu item. */
export const menuItemClass = `${menuItemBase} text-primary hover:bg-hover`

/** Destructive menu item (delete / trash). */
export const menuItemDangerClass = `${menuItemBase} text-rose-600 dark:text-rose-400 hover:bg-rose-50 dark:hover:bg-rose-950/25`

/** Base layout/typography only — use when an item needs custom color/active states. */
export { menuItemBase }
