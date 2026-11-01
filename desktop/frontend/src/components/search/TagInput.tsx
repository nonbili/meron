import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { loadTagCounts, type TagCount } from '../../states/mailTags'

const MAX_SUGGESTIONS = 8

/** Where the tag being typed starts: the field lists tags by spaces or commas. */
function tailStart(value: string) {
  return Math.max(value.lastIndexOf(' '), value.lastIndexOf(','), value.lastIndexOf('\t')) + 1
}

function listedTags(value: string) {
  return value.split(/[\s,]+/).filter(Boolean)
}

/** Known tags completing the one being typed, most used first, leaving out those already listed. */
export function tagSuggestions(known: TagCount[], value: string): string[] {
  const start = tailStart(value)
  const tail = value.slice(start).toLowerCase()
  const listed = new Set(
    value
      .slice(0, start)
      .toLowerCase()
      .split(/[\s,]+/),
  )
  return known
    .filter(({ tag }) => tag.includes(tail) && !listed.has(tag))
    .sort((a, b) => b.count - a.count)
    .slice(0, MAX_SUGGESTIONS)
    .map(({ tag }) => tag)
}

/** The field with the tag being typed replaced by `tag`, ready for the next one. */
export function completeTag(value: string, tag: string) {
  return `${value.slice(0, tailStart(value))}${tag} `
}

/** The field without the tag listed at `index`, keeping what is being typed after them. */
export function removeTag(value: string, index: number, tail = '') {
  const rest = listedTags(value).filter((_, at) => at !== index)
  return `${rest.map((tag) => `${tag} `).join('')}${tail}`
}

/**
 * The search form's Tags field, suggesting known tags. Its value stays the plain
 * list the search syntax reads: tags ended by a separator show as chips, and the
 * input holds only the one being typed. Away from the field every tag is a chip.
 * Enter searches unless the arrow keys picked a suggestion, as a tag typed in
 * full needs no completing.
 */
export function TagInput({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const { t } = useTranslation()
  const [known, setKnown] = useState<TagCount[]>([])
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(-1)
  const inputRef = useRef<HTMLInputElement | null>(null)

  useEffect(() => {
    void loadTagCounts().then(setKnown, () => setKnown([]))
  }, [])

  const start = open ? tailStart(value) : value.length
  const tail = value.slice(start)
  const chips = listedTags(value.slice(0, start))
  const suggestions = open ? tagSuggestions(known, value) : []
  const pick = (tag: string) => {
    onChange(completeTag(value, tag))
    setActive(-1)
  }

  return (
    <div className="relative min-w-0 flex-1">
      <div
        onClick={() => inputRef.current?.focus()}
        className="flex min-h-7 w-full cursor-text flex-wrap items-center gap-1 rounded-lg bg-app px-2 py-1 text-xs text-primary ring-1 ring-border focus-within:ring-accent"
      >
        {chips.map((tag, index) => (
          <span
            key={`${index}:${tag}`}
            className="flex min-w-0 max-w-full items-center gap-0.5 rounded bg-accent/10 pr-0.5 pl-1.5 text-[0.6875rem] font-medium leading-5 text-accent"
          >
            <span className="min-w-0 truncate" title={tag}>
              {tag}
            </span>
            <button
              type="button"
              aria-label={t('tags.remove', { tag })}
              title={t('tags.remove', { tag })}
              // Keeps the caret in the field, and the field where the press found it.
              onMouseDown={(event) => event.preventDefault()}
              onClick={(event) => {
                event.stopPropagation()
                onChange(removeTag(value.slice(0, start), index, tail))
              }}
              className="flex shrink-0 cursor-pointer rounded p-0.5 hover:bg-accent/15"
            >
              <X size={11} />
            </button>
          </span>
        ))}
        <input
          ref={inputRef}
          aria-label={t('tags.title')}
          value={tail}
          onChange={(event) => {
            onChange(`${value.slice(0, start)}${event.target.value}`)
            setOpen(true)
            setActive(-1)
          }}
          onFocus={() => {
            // A tag left unfinished, or read from the query, stays a chip.
            if (tailStart(value) < value.length) onChange(`${value} `)
            setOpen(true)
          }}
          onBlur={() => setOpen(false)}
          onKeyDown={(event) => {
            if (event.key === 'Backspace' && !tail && chips.length > 0) {
              event.preventDefault()
              onChange(removeTag(value, chips.length - 1))
              setActive(-1)
              return
            }
            if (suggestions.length === 0) return
            if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
              event.preventDefault()
              const step = event.key === 'ArrowDown' ? 1 : suggestions.length - 1
              setActive((index) =>
                index < 0 && step !== 1 ? suggestions.length - 1 : (index + step) % suggestions.length,
              )
            } else if (event.key === 'Enter' && active >= 0) {
              event.preventDefault()
              pick(suggestions[active])
            }
          }}
          className="h-5 min-w-16 flex-1 border-0 bg-transparent p-0 text-xs text-primary outline-none"
        />
      </div>
      {suggestions.length > 0 && (
        <ul className="absolute left-0 right-0 top-full z-50 mt-1 max-h-60 overflow-y-auto rounded-lg border border-border bg-chats py-1 shadow-xl">
          {suggestions.map((tag, index) => (
            <li
              key={tag}
              className={`cursor-pointer truncate px-2 py-1 text-xs text-primary ${index === active ? 'bg-hover' : ''}`}
              onMouseDown={(event) => {
                event.preventDefault()
                pick(tag)
              }}
            >
              {tag}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
