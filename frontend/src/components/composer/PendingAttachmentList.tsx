import { X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { invoke } from '../../lib/bridge'
import type { ComposerAttachment } from '../../types'
import { fileIconFor, formatFileSize } from '../chat/messageHelpers'

export function composerAttachmentSrc(att: ComposerAttachment): string {
  return `data:${att.mime};base64,${att.data}`
}

function isPreviewableImage(att: ComposerAttachment): boolean {
  return att.mime.toLowerCase().startsWith('image/') && !!att.data
}

async function openAttachment(att: ComposerAttachment, onPreviewImage: (attachment: ComposerAttachment) => void) {
  if (isPreviewableImage(att)) {
    onPreviewImage(att)
    return
  }
  await invoke('composer.openAttachment', {
    filename: att.filename,
    mime: att.mime,
    data: att.data,
  })
}

export function PendingAttachmentList({
  attachments,
  onRemove,
  onPreviewImage,
}: {
  attachments: ComposerAttachment[]
  onRemove: (id: string) => void
  onPreviewImage: (attachment: ComposerAttachment) => void
}) {
  const { t } = useTranslation()

  return (
    <>
      {attachments.map((att) => {
        const AttIcon = fileIconFor(att.filename, att.mime)
        return (
          <div
            role="button"
            tabIndex={0}
            key={att.id}
            onClick={() => void openAttachment(att, onPreviewImage)}
            onKeyDown={(event) => {
              if (event.key !== 'Enter' && event.key !== ' ') return
              event.preventDefault()
              void openAttachment(att, onPreviewImage)
            }}
            className="flex items-center gap-1.5 rounded-xl border border-border/40 bg-chats px-2.5 py-1.5 shadow-xs max-w-[200px] text-left transition-colors hover:bg-hover cursor-pointer"
            title={att.filename}
          >
            <AttIcon size={13} className="shrink-0 text-accent" />
            <div className="flex min-w-0 flex-1 flex-col">
              <span className="truncate text-[11px] font-bold text-primary">{att.filename}</span>
              <span className="text-[9px] text-secondary">{formatFileSize(att.size)}</span>
            </div>
            <button
              type="button"
              onClick={(event) => {
                event.stopPropagation()
                onRemove(att.id)
              }}
              className="ml-1 shrink-0 rounded-full p-0.5 text-secondary transition-colors hover:bg-active cursor-pointer"
              title={t('composer.actions.removeAttachment')}
            >
              <X size={12} />
            </button>
          </div>
        )
      })}
    </>
  )
}
