const MAX_CHIPS = 3

// A thread's tags, on their own line of a list or board row, so they leave the
// subject its room. Past a few of them a count stands in.
export function TagChips({ tags }: { tags?: string[] }) {
  if (!tags?.length) return null
  const hidden = tags.length - MAX_CHIPS
  return (
    <span className="flex min-w-0 items-center gap-1" title={tags.join(', ')}>
      {tags.slice(0, MAX_CHIPS).map((tag) => (
        <span
          key={tag}
          className="min-w-0 max-w-40 truncate rounded bg-accent/10 px-1 text-[0.625rem] font-medium leading-4 text-accent"
        >
          {tag}
        </span>
      ))}
      {hidden > 0 && <span className="shrink-0 text-[0.625rem] font-medium leading-4 text-secondary">+{hidden}</span>}
    </span>
  )
}
