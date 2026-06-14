import { forwardRef } from 'react'
import type { ButtonHTMLAttributes, ReactNode } from 'react'
import type { LucideIcon } from 'lucide-react'
import { clsx } from '../../lib/utils'

type IconButtonVariant = 'ghost' | 'accent' | 'accentSoft' | 'danger'
type IconButtonSize = 'sm' | 'md' | 'lg'
type IconButtonRadius = 'full' | 'lg' | 'xl'

const SIZES: Record<IconButtonSize, { box: string; icon: number }> = {
  sm: { box: 'h-7 w-7', icon: 14 },
  md: { box: 'h-8 w-8', icon: 15 },
  lg: { box: 'h-9 w-9', icon: 16 },
}

const RADII: Record<IconButtonRadius, string> = {
  full: 'rounded-full',
  lg: 'rounded-lg',
  xl: 'rounded-xl',
}

function variantClasses(variant: IconButtonVariant, active: boolean): string {
  switch (variant) {
    case 'accent':
      return 'bg-accent text-white shadow-sm shadow-accent/20 hover:bg-accent-hover'
    case 'accentSoft':
      return 'bg-accent/10 text-accent hover:bg-accent/15'
    case 'danger':
      return 'text-rose-600 hover:bg-rose-50 dark:text-rose-400 dark:hover:bg-rose-950/30'
    case 'ghost':
    default:
      return active ? 'bg-active text-primary' : 'text-secondary hover:bg-hover hover:text-primary'
  }
}

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  // Accessible label — drives both the tooltip and aria-label. Required so we
  // never ship an unlabeled icon-only control.
  label: string
  icon?: LucideIcon
  iconSize?: number
  children?: ReactNode
  variant?: IconButtonVariant
  size?: IconButtonSize
  radius?: IconButtonRadius
  active?: boolean
}

// Square, icon-only button. Centralizes the rounded-full + slate hover treatment
// repeated across the headers so the hover token and
// radius can't drift per call site. Spreads through refs, dnd-kit listeners,
// onPointerDown, disabled, etc.; `className` is additive (see clsx note).
export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  {
    label,
    icon: Icon,
    iconSize,
    children,
    variant = 'ghost',
    size = 'lg',
    radius = 'full',
    active = false,
    className,
    type,
    ...rest
  },
  ref,
) {
  const sizing = SIZES[size]
  return (
    <button
      ref={ref}
      type={type ?? 'button'}
      title={label}
      aria-label={label}
      className={clsx(
        'flex shrink-0 items-center justify-center cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-50',
        sizing.box,
        RADII[radius],
        variantClasses(variant, active),
        className,
      )}
      {...rest}
    >
      {Icon ? <Icon size={iconSize ?? sizing.icon} /> : children}
    </button>
  )
})
