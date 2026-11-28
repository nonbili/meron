import type { DragEvent } from 'react'
import { attachmentSaveFilename } from './attachmentFilename'
import { invoke } from './bridge'

export type DraggableAttachment = {
  filename: string
  key?: string | null
  mime?: string
}

export function attachmentDragProps(attachment?: DraggableAttachment) {
  const key = attachment?.key
  if (!key || !attachment || !(window as any).go?.main?.App?.Invoke) return {}

  return {
    draggable: true,
    onDragStart(event: DragEvent<HTMLElement>) {
      event.preventDefault()
      event.stopPropagation()
      const filename = attachmentSaveFilename(attachment.filename, key, attachment.mime)
      void invoke('mail.dragAttachment', { key, filename }).catch((error) => {
        console.error('Failed to drag attachment:', error)
      })
    },
  }
}
