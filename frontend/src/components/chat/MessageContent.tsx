import { Download, Image } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { downloadAttachment } from '../../states/mail'
import { revealRemote, thread$ } from '../../states/thread'
import type { Message } from '../../types'
import { fileIconFor, formatFileSize, mediaSrc } from './messageHelpers'
import { MessageBubbleBody } from './MessageBubbleBody'
import { VideoAttachment } from './VideoAttachment'
import type { MessageView } from './useMessageView'

// Everything below a message's header: image attachments, videos, the
// hidden-remote-images affordance, the body and the file attachment list.
// Shared by both conversation layouts (see useMessageView).
export function MessageContent({
  message,
  view,
  galleryOffset,
  fullHeight = false,
  onLinkHover,
  onUserScrollIntent,
}: {
  message: Message
  view: MessageView
  // Index of this message's first image within the thread-wide gallery list.
  galleryOffset: number
  /** Let the body grow instead of scrolling inside its own box. */
  fullHeight?: boolean
  onLinkHover?: (url: string | null) => void
  onUserScrollIntent?: () => void
}) {
  const { t } = useTranslation()
  const { attachmentImages, bubbleAttachmentImages, videos, hiddenRemoteCount, files } = view
  const onOpenImage = (idx: number) => thread$.galleryIndex.set(idx)

  return (
    <>
      {/* Image attachments */}
      {bubbleAttachmentImages.length > 0 &&
        (() => {
          const count = bubbleAttachmentImages.length
          let gridClass = 'grid-cols-2 max-w-[320px]'
          let btnClass = 'h-40'

          if (count === 1) {
            gridClass = 'grid-cols-1 max-w-[380px]'
            btnClass = 'h-56'
          } else if (count === 2) {
            gridClass = 'grid-cols-2 max-w-[480px]'
            btnClass = 'h-40'
          } else if (count === 3) {
            gridClass = 'grid-cols-3 max-w-[540px]'
            btnClass = 'h-32'
          } else if (count >= 4) {
            gridClass = 'grid-cols-4 max-w-[640px]'
            btnClass = 'h-28'
          }

          return (
            <div className={`mb-2 grid gap-1.5 rounded-lg overflow-hidden border border-border/20 ${gridClass}`}>
              {bubbleAttachmentImages.map((image, idx) => (
                <button
                  key={idx}
                  type="button"
                  onClick={() => onOpenImage(galleryOffset + attachmentImages.indexOf(image))}
                  className={`block w-full overflow-hidden hover:opacity-90 cursor-pointer ${btnClass}`}
                  title={image.filename}
                >
                  <img
                    src={mediaSrc(image)}
                    alt={image.filename}
                    loading="lazy"
                    className="w-full h-full object-cover object-top"
                  />
                </button>
              ))}
            </div>
          )
        })()}

      {/* Video attachments: rendered with native controls, played from disk
          (cached) or straight from the remote URL. The corner button opens the
          source in the system player as a fallback when the in-app webview
          can't decode the codec (common on Linux/WebKitGTK). */}
      {videos.length > 0 && (
        <div className="mb-2 flex flex-col gap-1.5">
          {videos.map((video, idx) => (
            <VideoAttachment
              key={idx}
              src={mediaSrc(video)}
              externalUrl={video.url ?? mediaSrc(video)}
              externalLabel={t('chat.openExternalPlayer')}
            />
          ))}
        </div>
      )}

      {/* Hidden remote images: let the user reveal them for this message */}
      {hiddenRemoteCount > 0 && (
        <button
          onClick={() => revealRemote(message.id)}
          className="mb-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed border-border/50 bg-black/[0.02] dark:bg-white/[0.02] py-2 text-[0.6875rem] font-semibold text-secondary hover:text-accent hover:border-accent/40 cursor-pointer transition-colors"
        >
          <Image size={13} />
          {t('chat.showImages', { count: hiddenRemoteCount })}
        </button>
      )}

      <MessageBubbleBody
        message={message}
        useHtmlBody={view.useHtmlBody}
        normalizedSearchQuery={view.normalizedSearchQuery}
        activeSearchMatch={view.activeSearchMatch}
        fullHeight={fullHeight}
        onLinkHover={onLinkHover}
        onUserScrollIntent={onUserScrollIntent}
      />

      {/* File attachments — click to save via native dialog (when on disk) */}
      {files.map((file, idx) => {
        const downloadable = !!file.key
        const FileIcon = fileIconFor(file.filename, file.mime)
        return (
          <button
            key={idx}
            type="button"
            disabled={!downloadable}
            onClick={() => downloadAttachment(file)}
            title={downloadable ? t('chat.saveFile', { filename: file.filename }) : file.filename}
            className={`group mt-2.5 flex w-full items-center gap-2 rounded-xl bg-black/[0.03] dark:bg-white/[0.03] p-2 text-xs font-semibold border border-border/20 text-left ${
              downloadable ? 'hover:bg-black/[0.06] dark:hover:bg-white/[0.06] cursor-pointer' : 'cursor-default'
            }`}
          >
            <FileIcon size={15} className="text-accent shrink-0" />
            <span className="truncate">{file.filename}</span>
            <span className="text-[0.59375rem] text-secondary ml-auto shrink-0 font-normal">
              {formatFileSize(file.size)}
            </span>
            {downloadable && (
              <Download
                size={13}
                className="text-secondary shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
              />
            )}
          </button>
        )
      })}
    </>
  )
}
