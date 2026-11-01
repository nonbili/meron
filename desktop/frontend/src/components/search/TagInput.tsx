import { useEffect, useState } from 'react'
import { loadTagCounts, type TagCount } from '../../states/mailTags'

const MAX_SUGGESTIONS = 8

/** Where the tag being typed starts: the field lists tags by spaces or commas. */
function tailStart(value: string) {
  return Math.max(value.lastIndexOf(' '), value.lastIndexOf(','), value.lastIndexOf('\t')) + 1
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

/**
 * The search form's Tags field, suggesting known tags. Enter searches unless the
 * arrow keys picked a suggestion, as a tag typed in full needs no completing.
 */
export function TagInput({
  value,
  onChange,
  className,
}: {
  value: string
  onChange: (value: string) => void
  className: string
}) {
  const [known, setKnown] = useState<TagCount[]>([])
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(-1)

  useEffect(() => {
    void loadTagCounts().then(setKnown, () => setKnown([]))
  }, [])

  const suggestions = open ? tagSuggestions(known, value) : []
  const pick = (tag: string) => {
    onChange(completeTag(value, tag))
    setActive(-1)
  }

  return (
    <div className="relative min-w-0 flex-1">
      <input
        value={value}
        onChange={(event) => {
          onChange(event.target.value)
          setOpen(true)
          setActive(-1)
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onKeyDown={(event) => {
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
        className={className}
      />
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
