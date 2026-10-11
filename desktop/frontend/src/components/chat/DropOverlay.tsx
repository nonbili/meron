import { ImagePlus, Paperclip } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'

/** Shown while files are dragged over a drop area (useFileDrop): "attach" over a
 *  conversation or the composer, "inline" over the composer's message body. It
 *  is the drop surface itself: it covers what is beneath, message bodies in
 *  iframes and the reply box included, which would otherwise take the drag away
 *  from the pane. Its events bubble to the area's drop handlers. */
export function DropOverlay({
  kind = 'attach',
  className = 'inset-2',
}: {
  kind?: 'attach' | 'inline'
  className?: string
}) {
  const { t } = useTranslation()
  const Icon = kind === 'inline' ? ImagePlus : Paperclip
  return (
    // The outer layer catches the drag across the whole area; the dashed box is
    // drawn inside it.
    <div className="absolute inset-0 z-30">
      <div
        className={`absolute flex items-center justify-center rounded-2xl border-2 border-dashed border-accent bg-chat/85 ${className}`}
      >
        <div className="flex items-center gap-2 px-4 text-center text-sm font-semibold text-accent">
          <Icon size={16} />
          <span>{kind === 'inline' ? t('composer.dropToInsertInline') : t('chat.dropToAttach')}</span>
        </div>
      </div>
    </div>
  )
}
