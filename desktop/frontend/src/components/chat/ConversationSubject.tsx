import { useId, useState } from 'react'
import { clsx } from '../../lib/utils'

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
  // The tooltip only earns its place when the heading is cut off; a subject that
  // fits would just be repeated underneath itself.
  const [truncated, setTruncated] = useState(false)
  const measure = (element: HTMLElement) => setTruncated(element.scrollWidth > element.clientWidth)

  return (
    <h2 className="group/subject relative min-w-0">
      <button
        type="button"
        onClick={onCopy}
        onMouseEnter={(event) => measure(event.currentTarget)}
        onFocus={(event) => measure(event.currentTarget)}
        className="block max-w-full truncate rounded-sm text-left text-[0.96875rem] font-bold leading-snug text-primary outline-none transition-colors cursor-copy hover:text-accent focus-visible:ring-2 focus-visible:ring-accent/40"
        aria-label={copyLabel}
        aria-describedby={tooltipId}
      >
        {subject}
      </button>
      <span
        id={tooltipId}
        role="tooltip"
        // Sized to the subject, capped at the header's width, so a short one gets
        // a short bubble rather than a full-width bar.
        className={clsx(
          'pointer-events-none invisible absolute top-[calc(100%+1.5rem)] left-0 z-50 w-max max-w-full break-words whitespace-normal rounded-xl border border-border bg-chats px-3 py-2 text-left text-xs font-semibold leading-snug text-primary opacity-0 shadow-lg transition-opacity',
          truncated &&
            'group-hover/subject:visible group-hover/subject:opacity-100 group-focus-within/subject:visible group-focus-within/subject:opacity-100',
        )}
      >
        <span className="block select-text">{subject}</span>
      </span>
    </h2>
  )
}
