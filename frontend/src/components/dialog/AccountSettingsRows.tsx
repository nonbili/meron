import type { ReactNode } from 'react'
import { ChevronDown } from 'lucide-react'
import { InfoTip } from '../tooltip/InfoTip'

// iOS/chat-style switch, matching the look used across the settings UI.
function Switch({ checked, onChange }: { checked: boolean; onChange: () => void }) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      onClick={onChange}
      className={`relative inline-flex h-5.5 w-10 shrink-0 cursor-pointer rounded-full border border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
        checked ? 'bg-accent' : 'bg-active'
      }`}
    >
      <span
        className={`pointer-events-none inline-block h-4.5 w-4.5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${
          checked ? 'translate-x-4.5' : 'translate-x-0.2'
        } relative top-[1px]`}
      />
    </button>
  )
}

// A settings section: compact heading + one card whose rows are separated by
// thin dividers. Rows inside should be SettingRow-based.
export function SettingsGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-2 px-1 text-[12px] font-semibold text-secondary">{title}</h3>
      <div className="rounded-2xl bg-raised/80 border border-border/60 divide-y divide-border/40 overflow-hidden shadow-sm shadow-black/[0.03] dark:shadow-black/10">
        {children}
      </div>
    </section>
  )
}

// One flat row inside a SettingsGroup card: title (+ optional info tooltip) on
// the left, the control on the right. No inline hint text.
export function SettingRow({
  title,
  hint,
  control,
}: {
  icon?: ReactNode
  title: string
  hint?: string
  control: ReactNode
}) {
  return (
    <div className="flex min-h-11 items-center justify-between gap-4 px-3.5 py-2">
      <div className="flex min-w-0 items-center gap-2.5">
        <span className="truncate text-xs font-normal text-primary">{title}</span>
        <InfoTip hint={hint} />
      </div>
      <div className="flex shrink-0 items-center">{control}</div>
    </div>
  )
}

export function ToggleRow({
  icon,
  title,
  hint,
  checked,
  onChange,
}: {
  icon?: ReactNode
  title: string
  hint?: string
  checked: boolean
  onChange: () => void
}) {
  return <SettingRow icon={icon} title={title} hint={hint} control={<Switch checked={checked} onChange={onChange} />} />
}

export function SegmentedRow<T extends string>({
  icon,
  title,
  hint,
  value,
  options,
  onChange,
}: {
  icon?: ReactNode
  title: string
  hint?: string
  value: T
  options: { value: T; label: string }[]
  onChange: (value: T) => void
}) {
  return (
    <SettingRow
      icon={icon}
      title={title}
      hint={hint}
      control={
        <div className="flex items-center gap-0.5 rounded-lg bg-active/70 p-0.5">
          {options.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => onChange(opt.value)}
              className={`rounded-md px-2.5 py-1 text-xs font-semibold transition-colors cursor-pointer ${
                value === opt.value ? 'bg-chats text-accent shadow-sm' : 'text-secondary hover:text-primary'
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
      }
    />
  )
}

export function NumberRow({
  icon,
  title,
  hint,
  value,
  min,
  max,
  step,
  suffix,
  onChange,
}: {
  icon?: ReactNode
  title: string
  hint?: string
  value: string
  min: number
  max: number
  step: number
  suffix: string
  onChange: (value: string) => void
}) {
  return (
    <SettingRow
      icon={icon}
      title={title}
      hint={hint}
      control={
        <label className="flex items-center gap-1.5">
          <input
            type="number"
            min={min}
            max={max}
            step={step}
            value={value}
            onChange={(event) => onChange(event.target.value)}
            className="w-20 text-xs font-semibold bg-chats text-primary px-2.5 py-1 rounded-lg border border-border focus:outline-none focus:border-accent transition-colors"
          />
          <span className="text-[10.5px] font-bold text-secondary">{suffix}</span>
        </label>
      }
    />
  )
}

export function TextRow({
  icon,
  title,
  hint,
  value,
  placeholder,
  onChange,
}: {
  icon?: ReactNode
  title: string
  hint?: string
  value: string
  placeholder?: string
  onChange: (value: string) => void
}) {
  return (
    <SettingRow
      icon={icon}
      title={title}
      hint={hint}
      control={
        <input
          type="text"
          value={value}
          placeholder={placeholder}
          onChange={(event) => onChange(event.target.value)}
          className="w-44 text-xs font-semibold bg-chats text-primary px-2.5 py-1 rounded-lg border border-border focus:outline-none focus:border-accent transition-colors"
        />
      }
    />
  )
}

export function SelectRow({
  icon,
  title,
  hint,
  value,
  options,
  onChange,
}: {
  icon?: ReactNode
  title: string
  hint?: string
  value: string
  options: { value: string; label: string }[]
  onChange: (value: string) => void
}) {
  return (
    <SettingRow
      icon={icon}
      title={title}
      hint={hint}
      control={
        <div className="relative flex items-center">
          <select
            value={value}
            onChange={(event) => onChange(event.target.value)}
            className="appearance-none text-xs font-semibold bg-chats text-primary pl-3 pr-8 py-1.5 rounded-xl border border-border focus:outline-none focus:border-accent transition-colors cursor-pointer outline-none w-44"
          >
            {options.map((opt) => (
              <option key={opt.value} value={opt.value} className="bg-chats text-primary">
                {opt.label}
              </option>
            ))}
          </select>
          <div className="pointer-events-none absolute right-2.5 flex items-center text-secondary">
            <ChevronDown size={12} />
          </div>
        </div>
      }
    />
  )
}
