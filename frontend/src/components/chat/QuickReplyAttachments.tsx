import { X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { compose$ } from '../../states/compose'
import type { ComposerAttachment } from '../../types'
import { fileIconFor, formatFileSize } from './messageHelpers'

// Preview deck of the quick-reply's pending attachments. Renders nothing empty.
export function QuickReplyAttachments({ attachments }: { attachments: ComposerAttachment[] }) {
  const { t } = useTranslation()
  if (attachments.length === 0) return null

  return (
    <div className="flex flex-wrap gap-2 mb-1 p-1 bg-black/[0.02] dark:bg-white/[0.02] rounded-xl border border-border/10 max-h-40 overflow-y-auto select-none">
      {attachments.map((att) => {
        const AttIcon = fileIconFor(att.filename, att.mime)
        return (
          <div
            key={att.id}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-chats border border-border/40 shadow-xs max-w-[200px]"
          >
            <AttIcon size={13} className="text-accent shrink-0" />
            <div className="flex flex-col min-w-0 flex-1">
              <span className="text-[11px] font-bold text-primary truncate">{att.filename}</span>
              <span className="text-[9px] text-secondary">{formatFileSize(att.size)}</span>
            </div>
            <button
              onClick={() => compose$.composerAttachments.set(attachments.filter((a) => a.id !== att.id))}
              className="ml-1 p-0.5 rounded-full hover:bg-hover text-secondary cursor-pointer shrink-0 transition-colors"
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
