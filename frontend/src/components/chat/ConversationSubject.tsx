import { useId } from 'react'

export function ConversationSubject({
  subject,
  copyLabel,
  onCopy,
}: {
  subject: string
  copyLabel: string
  onCopy: () => void
}) {
  const tooltipId = useId()

  return (
    <h2 className="group/subject relative min-w-0">
      <button
        type="button"
        onClick={onCopy}
        className="block max-w-full truncate rounded-sm text-left text-[15.5px] font-bold leading-snug tracking-wide text-primary outline-none transition-colors cursor-copy hover:text-accent focus-visible:ring-2 focus-visible:ring-accent/40"
        aria-label={copyLabel}
        aria-describedby={tooltipId}
      >
        {subject}
      </button>
      <span
        id={tooltipId}
        role="tooltip"
        className="pointer-events-none invisible absolute top-[calc(100%+1.75rem)] left-0 z-50 w-full break-words whitespace-normal rounded-xl border border-border bg-raised px-3 py-2 text-left text-xs font-semibold leading-snug text-primary opacity-0 shadow-lg transition-opacity group-hover/subject:visible group-hover/subject:opacity-100 group-focus-within/subject:visible group-focus-within/subject:opacity-100"
      >
        <span className="block select-text">{subject}</span>
      </span>
    </h2>
  )
}
