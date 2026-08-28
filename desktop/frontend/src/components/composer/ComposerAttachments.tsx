import { useState } from 'react'
import type { ComposerAttachment } from '../../types'
import { AttachmentImagePreview } from './AttachmentImagePreview'
import { PendingAttachmentList } from './PendingAttachmentList'

// The chip row of pending attachments below the body. Renders nothing when empty.
export function ComposerAttachments({
  attachments,
  onRemove,
}: {
  attachments: ComposerAttachment[]
  onRemove: (id: string) => void
}) {
  const [previewAttachment, setPreviewAttachment] = useState<ComposerAttachment | null>(null)
  const visibleAttachments = attachments.filter((att) => !att.inlineId)

  if (visibleAttachments.length === 0) return null

  return (
    <>
      <div className="flex shrink-0 flex-wrap gap-2 border-t border-border px-4 py-2 select-none">
        <PendingAttachmentList
          attachments={visibleAttachments}
          onRemove={onRemove}
          onPreviewImage={setPreviewAttachment}
        />
      </div>
      {previewAttachment && (
        <AttachmentImagePreview attachment={previewAttachment} onClose={() => setPreviewAttachment(null)} />
      )}
    </>
  )
}
