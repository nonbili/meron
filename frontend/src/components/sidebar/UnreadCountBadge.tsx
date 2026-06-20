/**
 * Numeric inbox unread-count badge overlaid on the top-right of a sidebar avatar
 * or button. Caps at "99+". Renders nothing when count is 0.
 */
export function UnreadCountBadge({ count }: { count: number }) {
  if (count <= 0) return null
  return (
    <span className="pointer-events-none absolute -top-1 -right-1 flex h-[18px] min-w-[18px] items-center justify-center rounded-full bg-accent px-1 text-[10px] font-semibold leading-none text-white ring-2 ring-sidebar">
      {count > 99 ? '99+' : count}
    </span>
  )
}
