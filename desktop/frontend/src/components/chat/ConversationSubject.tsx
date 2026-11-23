import { copyText } from '../../lib/native'
import { showToast } from '../../states/ui'

// The conversation's subject, heading the message list so it scrolls away with
// the messages (the header above is only a toolbar). It wraps rather than
// truncating, so the whole subject is always readable. A click copies it.
export function ConversationSubject({
  subject,
  subtitle,
  copyLabel,
  copiedLabel,
}: {
  subject: string
  /** A line under the subject: an RSS item's feed host. */
  subtitle?: string
  copyLabel: string
  copiedLabel: string
}) {
  const onCopy = () => {
    const value = subject.trim()
    if (!value) return
    copyText(value)
      .then(() => showToast(copiedLabel))
      .catch(() => undefined)
  }
  return (
    <div className="px-1 pb-1 select-none">
      {/* The heading reads as the subject; the button in it as what it does. */}
      <h2 className="min-w-0" aria-label={subject}>
        <button
          type="button"
          onClick={onCopy}
          title={copyLabel}
          aria-label={copyLabel}
          className="block max-w-full break-words rounded-sm text-left text-[1.25rem] font-bold leading-snug text-primary outline-none transition-colors cursor-copy hover:text-accent focus-visible:ring-2 focus-visible:ring-accent/40"
        >
          {subject}
        </button>
      </h2>
      {subtitle && (
        <p className="mt-0.5 truncate text-xs font-medium text-secondary" title={subtitle}>
          {subtitle}
        </p>
      )}
    </div>
  )
}
