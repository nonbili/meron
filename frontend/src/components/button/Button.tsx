import { forwardRef } from 'react'
import type { ButtonHTMLAttributes, ReactNode } from 'react'
import type { LucideIcon } from 'lucide-react'
import { clsx } from '../../lib/utils'

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger'
type ButtonSize = 'sm' | 'md'

const SIZES: Record<ButtonSize, { box: string; icon: number }> = {
  sm: { box: 'h-8 px-3 text-[11px]', icon: 13 },
  md: { box: 'h-9 px-3.5 text-xs', icon: 14 },
}

const VARIANTS: Record<ButtonVariant, string> = {
  primary:
    'bg-accent text-white shadow-md shadow-accent/15 hover:bg-accent-hover hover:shadow-lg hover:shadow-accent/20 active:scale-98',
  secondary: 'border border-border/70 bg-chats text-primary shadow-sm hover:bg-hover active:scale-95',
  ghost: 'text-secondary hover:bg-hover hover:text-primary',
  danger: 'bg-rose-50 text-rose-600 hover:bg-rose-100 dark:bg-rose-950/20 dark:text-rose-400 dark:hover:bg-rose-950/40',
}

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant
  size?: ButtonSize
  leftIcon?: LucideIcon
  rightIcon?: LucideIcon
  children: ReactNode
}

// Labeled button. Variants mirror the styles already in use across dialogs and
// headers (accent CTA, bordered secondary, ghost, soft-danger) so call sites stop
// re-deriving them by hand. `className` is additive (see clsx note).
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = 'primary', size = 'md', leftIcon: Left, rightIcon: Right, children, className, type, ...rest },
  ref,
) {
  const sizing = SIZES[size]
  return (
    <button
      ref={ref}
      type={type ?? 'button'}
      className={clsx(
        'inline-flex shrink-0 items-center justify-center gap-1.5 rounded-xl font-semibold whitespace-nowrap cursor-pointer transition-all duration-200 disabled:cursor-not-allowed disabled:opacity-50',
        sizing.box,
        VARIANTS[variant],
        className,
      )}
      {...rest}
    >
      {Left && <Left size={sizing.icon} className="stroke-[2.5]" />}
      {children}
      {Right && <Right size={sizing.icon} className="stroke-[2.5]" />}
    </button>
  )
})
