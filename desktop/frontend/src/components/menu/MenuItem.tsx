import type { ReactNode } from 'react'
import { menuItemClass, menuItemDangerClass } from './menuStyles'

/**
 * A row in a popover / context menu: leading icon, label, optional trailing
 * slot (chevron, spinner, shortcut). Owns the shared typography/layout via
 * {@link menuItemClass}; pass `danger` for destructive actions. For rows that
 * need bespoke behaviour (submenu hover/refs, active toggles) compose
 * `menuItemClass` directly instead of using this component.
 */
export function MenuItem({
  icon,
  label,
  onClick,
  danger,
  disabled,
  submenuOpen,
  pressed,
  trailing,
  className,
}: {
  icon?: ReactNode
  label: ReactNode
  onClick?: () => void
  danger?: boolean
  disabled?: boolean
  /** Pass the open state for rows that open a submenu. */
  submenuOpen?: boolean
  /** Pass the on/off state for rows that toggle in place. */
  pressed?: boolean
  /** Right-aligned slot (e.g. a chevron or shortcut hint). */
  trailing?: ReactNode
  /** Extra classes appended to the shared item class. */
  className?: string
}) {
  const base = danger ? menuItemDangerClass : menuItemClass
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-haspopup={submenuOpen !== undefined ? 'menu' : undefined}
      aria-expanded={submenuOpen}
      aria-pressed={pressed}
      className={`${base}${className ? ` ${className}` : ''}`}
    >
      {icon}
      {trailing ? <span className="min-w-0 flex-1 truncate text-left">{label}</span> : label}
      {trailing}
    </button>
  )
}
