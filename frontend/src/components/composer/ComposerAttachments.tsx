import { X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import type { ComposerAttachment } from '../../types'
import { fileIconFor, formatFileSize } from '../chat/messageHelpers'

// The chip row of pending attachments below the body. Renders nothing when empty.
export function ComposerAttachments({
  attachments,
  onRemove,
}: {
  attachments: ComposerAttachment[]
  onRemove: (id: string) => void
}) {
  const { t } = useTranslation()
  const visibleAttachments = attachments.filter((att) => !att.inlineId)

  if (visibleAttachments.length === 0) return null

  return (
    <div className="flex shrink-0 flex-wrap gap-2 border-t border-border px-4 py-2 select-none">
      {visibleAttachments.map((att) => {
        const AttIcon = fileIconFor(att.filename, att.mime)
        return (
          <div
            key={att.id}
            className="flex items-center gap-1.5 rounded-xl border border-border/40 bg-chats px-2.5 py-1.5 shadow-xs max-w-[200px]"
          >
            <AttIcon size={13} className="shrink-0 text-accent" />
            <div className="flex min-w-0 flex-1 flex-col">
              <span className="truncate text-[11px] font-bold text-primary">{att.filename}</span>
              <span className="text-[9px] text-secondary">{formatFileSize(att.size)}</span>
            </div>
            <button
              onClick={() => onRemove(att.id)}
              className="ml-1 shrink-0 rounded-full p-0.5 text-secondary transition-colors hover:bg-hover cursor-pointer"
              title={t('composer.actions.removeAttachment')}
            >
              <X size={12} />
            </button>
          </div>
        )
      })}
    </div>
  )
}
