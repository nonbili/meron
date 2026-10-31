const MAX_CHIPS = 2

// A thread's tags, in a row of a list or board. Past a couple of them a count
// stands in, so the subject beside them keeps its room.
export function TagChips({ tags }: { tags?: string[] }) {
  if (!tags?.length) return null
  const hidden = tags.length - MAX_CHIPS
  return (
    <span className="flex shrink-0 items-center gap-1" title={tags.join(', ')}>
      {tags.slice(0, MAX_CHIPS).map((tag) => (
        <span
          key={tag}
          className="max-w-20 truncate rounded bg-accent/10 px-1 text-[0.625rem] font-medium leading-4 text-accent"
        >
          {tag}
        </span>
      ))}
      {hidden > 0 && <span className="text-[0.625rem] font-medium leading-4 text-secondary">+{hidden}</span>}
    </span>
  )
}
