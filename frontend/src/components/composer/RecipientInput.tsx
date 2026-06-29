import { useEffect, useRef, useState } from 'react'
import type { Contact } from '../../types'
import { suggestContacts, formatContact } from '../../lib/contacts'

type RecipientInputProps = {
  value: string
  onChange: (value: string) => void
  accountId: string
  placeholder?: string
  autoFocus?: boolean
}

// A comma-separated recipient field carries multiple addresses; autocomplete
// only ever completes the token after the final comma. Returns the leading
// portion (including that comma) plus the active token being typed.
function splitTail(value: string): { head: string; tail: string } {
  const idx = value.lastIndexOf(',')
  if (idx === -1) return { head: '', tail: value }
  return { head: value.slice(0, idx + 1), tail: value.slice(idx + 1) }
}

const inputClass = 'w-full bg-transparent text-[13px] text-primary placeholder-secondary outline-none'

export function RecipientInput({ value, onChange, accountId, placeholder, autoFocus }: RecipientInputProps) {
  const [suggestions, setSuggestions] = useState<Contact[]>([])
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const focusedRef = useRef(false)
  const blurTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const tail = splitTail(value).tail.trim()

  // Fetch suggestions (debounced) while focused. An empty token surfaces the
  // top correspondents; otherwise we match what's been typed so far.
  useEffect(() => {
    if (!focusedRef.current) return
    let cancelled = false
    const timer = setTimeout(async () => {
      const results = await suggestContacts(accountId, tail)
      if (cancelled) return
      setSuggestions(results)
      setActive(0)
      setOpen(results.length > 0)
    }, 120)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [tail, accountId])

  function accept(contact: Contact) {
    const { head } = splitTail(value)
    const prefix = head ? `${head} ` : ''
    onChange(`${prefix}${formatContact(contact)}, `)
    setOpen(false)
    setSuggestions([])
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (open && suggestions.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setActive((i) => (i + 1) % suggestions.length)
        return
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setActive((i) => (i - 1 + suggestions.length) % suggestions.length)
        return
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault()
        accept(suggestions[active])
        return
      }
      if (e.key === 'Escape') {
        e.preventDefault()
        e.stopPropagation()
        setOpen(false)
        return
      }
    }
  }

  return (
    <div className="relative flex-1">
      <input
        autoFocus={autoFocus}
        value={value}
        placeholder={placeholder}
        spellCheck={false}
        className={inputClass}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={onKeyDown}
        onFocus={() => {
          focusedRef.current = true
          if (blurTimer.current) clearTimeout(blurTimer.current)
          if (suggestions.length > 0) setOpen(true)
          else
            void suggestContacts(accountId, tail).then((r) => {
              if (!focusedRef.current) return
              setSuggestions(r)
              setActive(0)
              setOpen(r.length > 0)
            })
        }}
        onBlur={() => {
          // Delay so a mousedown on a suggestion registers before we close.
          blurTimer.current = setTimeout(() => {
            focusedRef.current = false
            setOpen(false)
          }, 150)
        }}
      />
      {open && suggestions.length > 0 && (
        <ul className="absolute left-0 right-0 top-full z-50 mt-1 max-h-60 overflow-y-auto rounded-lg border border-border bg-chats py-1 shadow-xl">
          {suggestions.map((c, i) => (
            <li
              key={c.addr}
              // mousedown (not click) so it fires before the input's blur.
              onMouseDown={(e) => {
                e.preventDefault()
                accept(c)
              }}
              onMouseEnter={() => setActive(i)}
              className={`cursor-pointer px-3 py-1.5 text-[13px] ${i === active ? 'bg-accent/10' : ''}`}
            >
              {c.name.trim() && c.name.trim().toLowerCase() !== c.addr.toLowerCase() ? (
                <span>
                  <span className="text-primary">{c.name.trim()}</span>{' '}
                  <span className="text-secondary">{`<${c.addr}>`}</span>
                </span>
              ) : (
                <span className="text-primary">{c.addr}</span>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
