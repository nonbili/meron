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
