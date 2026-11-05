import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { Contact } from '../../types'
import { suggestContacts, formatContact } from '../../lib/contacts'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { normalizeRecipientInput, pasteRecipientInput, recipientSeparatorIndices } from '../../lib/recipientInput'

type RecipientInputProps = {
  value: string
  onChange: (value: string) => void
  accountId: string
  placeholder?: string
  autoFocus?: boolean
  inputRef?: React.Ref<HTMLInputElement>
  onTab?: () => void
  /** Bare addresses, as search's `from:`/`to:` take them: a pick adds the address, not `Name <address>`. */
  bare?: boolean
  className?: string
}

// A comma-separated recipient field carries multiple addresses; autocomplete
// only ever completes the token after the final comma — outside quotes, as a
// quoted name like `"Lee, Ann"` holds one. Returns the leading portion
// (including that comma) plus the active token being typed.
function splitTail(value: string): { head: string; tail: string } {
  const idx = recipientSeparatorIndices(value).at(-1) ?? -1
  if (idx === -1) return { head: '', tail: value }
  return { head: value.slice(0, idx + 1), tail: value.slice(idx + 1) }
}

const inputClass = 'w-full bg-transparent text-[0.8125rem] text-primary placeholder-secondary outline-none'

export function RecipientInput({
  value,
  onChange,
  accountId,
  placeholder,
  autoFocus,
  inputRef,
  onTab,
  bare = false,
  className = inputClass,
}: RecipientInputProps) {
  const [suggestions, setSuggestions] = useState<Contact[]>([])
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const focusedRef = useRef(false)
  const suggestionsEnabledRef = useRef(false)
  const blurTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const caretRef = useRef<{ input: HTMLInputElement; position: number } | null>(null)
  useLayoutEffect(() => {
    const caret = caretRef.current
    if (caret) {
      caret.input.setSelectionRange(caret.position, caret.position)
      caretRef.current = null
    }
  })

  const tail = splitTail(value).tail.trim()
  // The account and token the held suggestions were looked up for.
  const lookupKey = `${accountId}\n${tail}`
  const suggestionsKeyRef = useRef('')

  // Fetch suggestions (debounced) while focused and typing. An empty token
  // shows nothing: a list of top correspondents on every open composer only
  // covers the fields below it.
  useEffect(() => {
    if (!tail) {
      setOpen(false)
      setSuggestions([])
      suggestionsKeyRef.current = ''
      return
    }
    if (!focusedRef.current || !suggestionsEnabledRef.current) return
    let cancelled = false
    const timer = setTimeout(async () => {
      const results = await suggestContacts(accountId, tail)
      if (cancelled || !suggestionsEnabledRef.current) return
      // Kept even when focus has left, so coming back finds the lookup for
      // the token as it stands rather than one from before the last edit.
      suggestionsKeyRef.current = `${accountId}\n${tail}`
      setSuggestions(results)
      setActive(0)
      setOpen(focusedRef.current && results.length > 0)
    }, 120)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [tail, accountId])

  // The topmost Escape layer, so closing the list leaves a form or dialog around it open.
  useEscapeKey(() => setOpen(false), open && suggestions.length > 0)

  function accept(contact: Contact) {
    suggestionsEnabledRef.current = false
    const { head } = splitTail(value)
    const prefix = head ? `${head} ` : ''
    onChange(`${prefix}${bare ? contact.addr : formatContact(contact)}, `)
    setOpen(false)
    setSuggestions([])
    suggestionsKeyRef.current = ''
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    const plainTab = e.key === 'Tab' && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey
    if (e.key === 'Tab' && (!plainTab || !(open && suggestions.length > 0))) {
      suggestionsEnabledRef.current = false
      setOpen(false)
      if (onTab && plainTab) {
        e.preventDefault()
        onTab()
      }
      return
    }
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
    }
  }

  return (
    <div className="relative flex-1">
      <input
        ref={inputRef}
        autoFocus={autoFocus}
        value={value}
        placeholder={placeholder}
        spellCheck={false}
        className={className}
        onChange={(e) => {
          suggestionsEnabledRef.current = true
          const input = e.currentTarget
          const raw = input.value
          // Leave deletions and mid-field typing alone: users must be able to
          // backspace through separators and edit without losing their caret.
          const deleting = (e.nativeEvent as InputEvent).inputType?.startsWith('delete') || raw.length < value.length
          const atEnd = input.selectionStart === raw.length && input.selectionEnd === raw.length
          const event = e.nativeEvent as InputEvent
          const composing = event.isComposing || event.inputType === 'insertCompositionText'
          onChange(deleting || !atEnd || composing ? raw : normalizeRecipientInput(raw))
        }}
        onPaste={(e) => {
          // Read the clipboard before the single-line input removes tabs/newlines.
          e.preventDefault()
          const input = e.currentTarget
          const start = input.selectionStart ?? value.length
          const end = input.selectionEnd ?? start
          const pasted = pasteRecipientInput(value, start, end, e.clipboardData.getData('text'))
          suggestionsEnabledRef.current = true
          caretRef.current = { input, position: pasted.caret }
          onChange(pasted.value)
        }}
        onKeyDown={onKeyDown}
        onFocus={() => {
          focusedRef.current = true
          if (blurTimer.current) clearTimeout(blurTimer.current)
          // Focusing never starts suggesting; typing does (see onChange). Coming
          // back to a token that was mid-edit reopens what it had, though —
          // but only a lookup for that very token and account, never one the
          // edit overtook, or Tab would insert a recipient nobody typed.
          const resume = tail.length > 0 && suggestions.length > 0 && suggestionsKeyRef.current === lookupKey
          if (resume) suggestionsEnabledRef.current = true
          setOpen(resume)
        }}
        onBlur={() => {
          focusedRef.current = false
          // Delay so a mousedown on a suggestion registers before we close.
          blurTimer.current = setTimeout(() => {
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
              className={`cursor-pointer px-3 py-1.5 text-[0.8125rem] ${i === active ? 'bg-accent/10' : ''}`}
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
