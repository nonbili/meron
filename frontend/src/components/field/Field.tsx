import { forwardRef } from 'react'
import type { InputHTMLAttributes, SelectHTMLAttributes } from 'react'
import { ChevronDown } from 'lucide-react'
import { clsx } from '../../lib/utils'

interface FieldProps {
  label: string
  value: string
  onChange: (value: string) => void
  onBlur?: () => void
  onKeyDown?: (event: React.KeyboardEvent<HTMLInputElement>) => void
  type?: string
  inputClassName?: string
  labelClassName?: string
}

type FieldSize = 'sm' | 'md' | 'lg'
type FieldSurface = 'app' | 'chats' | 'hover' | 'raised' | 'transparent'

const INPUT_SIZES: Record<FieldSize, string> = {
  sm: 'rounded-lg px-2.5 py-1 text-xs',
  md: 'rounded-lg px-3 py-2 text-sm',
  lg: 'rounded-xl px-3.5 py-2.5 text-[13px]',
}

const INPUT_SURFACES: Record<FieldSurface, string> = {
  app: 'bg-app',
  chats: 'bg-chats',
  hover: 'bg-hover focus:bg-chats',
  raised: 'bg-raised focus:bg-chats',
  transparent: 'bg-transparent',
}

export interface TextInputProps extends InputHTMLAttributes<HTMLInputElement> {
  fieldSize?: FieldSize
  surface?: FieldSurface
  invalid?: boolean
}

export const TextInput = forwardRef<HTMLInputElement, TextInputProps>(function TextInput(
  { fieldSize = 'sm', surface = 'chats', invalid = false, className, ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      className={clsx(
        'min-w-0 border text-primary placeholder-secondary outline-none transition-colors disabled:cursor-not-allowed disabled:opacity-60',
        INPUT_SIZES[fieldSize],
        INPUT_SURFACES[surface],
        invalid ? 'border-rose-400 focus:border-rose-500' : 'border-border focus:border-accent',
        className,
      )}
      {...rest}
    />
  )
})

export interface SelectInputProps extends SelectHTMLAttributes<HTMLSelectElement> {
  fieldSize?: FieldSize
  surface?: FieldSurface
  wrapperClassName?: string
}

export const SelectInput = forwardRef<HTMLSelectElement, SelectInputProps>(function SelectInput(
  { fieldSize = 'sm', surface = 'chats', className, wrapperClassName, children, ...rest },
  ref,
) {
  return (
    <div className={clsx('relative flex items-center', wrapperClassName)}>
      <select
        ref={ref}
        className={clsx(
          'w-full min-w-0 appearance-none border border-border text-primary outline-none transition-colors cursor-pointer disabled:cursor-not-allowed disabled:opacity-60 focus:border-accent',
          INPUT_SIZES[fieldSize],
          INPUT_SURFACES[surface],
          'pr-8',
          className,
        )}
        {...rest}
      >
        {children}
      </select>
      <div className="pointer-events-none absolute right-2.5 flex items-center text-secondary">
        <ChevronDown size={12} />
      </div>
    </div>
  )
})

export function Field({
  label,
  value,
  onChange,
  onBlur,
  onKeyDown,
  type = 'text',
  inputClassName,
  labelClassName,
}: FieldProps) {
  return (
    <label className="flex flex-col gap-1.5 w-full">
      <span className={`pl-0.5 ${labelClassName ?? 'text-[11px] font-semibold text-secondary'}`}>
        {label}
      </span>
      <input
        type={type}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onBlur={onBlur}
        onKeyDown={onKeyDown}
        className={
          inputClassName ??
          'w-full text-xs py-2 px-3.5 rounded-xl border border-border bg-raised text-primary placeholder-secondary focus:ring-1 focus:ring-accent focus:border-transparent focus:bg-chats transition-all outline-none'
        }
      />
    </label>
  )
}
