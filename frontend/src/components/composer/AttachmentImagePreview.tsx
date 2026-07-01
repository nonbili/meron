import { useEffect } from 'react'
import { createPortal } from 'react-dom'
import { X } from 'lucide-react'
import type { ComposerAttachment } from '../../types'
import { composerAttachmentSrc } from './PendingAttachmentList'

export function AttachmentImagePreview({
  attachment,
  onClose,
}: {
  attachment: ComposerAttachment
  onClose: () => void
}) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [onClose])

  return createPortal(
    <div
      className="fixed inset-0 z-50 flex flex-col bg-black/90 backdrop-blur-sm select-none"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
    >
      <div className="flex shrink-0 items-center justify-between gap-3 px-4 py-3 text-white/90">
        <span className="min-w-0 truncate text-xs font-semibold">{attachment.filename}</span>
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation()
            onClose()
          }}
          className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full hover:bg-white/15 cursor-pointer transition-colors"
          title="Close"
        >
          <X size={20} />
        </button>
      </div>
      <div className="flex min-h-0 flex-1 items-center justify-center overflow-hidden px-4 pb-4">
        <img
          src={composerAttachmentSrc(attachment)}
          alt={attachment.filename}
          draggable={false}
          onClick={(event) => event.stopPropagation()}
          className="max-h-full max-w-full object-contain"
        />
      </div>
    </div>,
    document.body,
  )
}
