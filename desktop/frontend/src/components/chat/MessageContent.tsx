import { Download } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { downloadAttachment, openAttachment } from '../../states/mail'
import { thread$ } from '../../states/thread'
import type { Message } from '../../types'
import { fileIconFor, formatFileSize, mediaSrc } from './messageHelpers'
import { MessageBubbleBody } from './MessageBubbleBody'
import { VideoAttachment } from './VideoAttachment'
import type { MessageView } from './useMessageView'

// Everything below a message's header: image attachments, videos, the body and
// the file attachment list. Blocked remote content is revealed from the
// message's actions menu rather than a banner over every newsletter.
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
  const { attachmentImages, bubbleAttachmentImages, videos, files } = view
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

      <MessageBubbleBody
        message={message}
        useHtmlBody={view.useHtmlBody}
        outgoing={view.outgoing}
        allowRemote={view.remoteVisible}
        normalizedSearchQuery={view.normalizedSearchQuery}
        activeSearchOffset={view.activeSearchOffset}
        fullHeight={fullHeight}
        onLinkHover={onLinkHover}
        onUserScrollIntent={onUserScrollIntent}
      />

      {/* File attachments — click opens in the default app, the icon saves via
          the native dialog (both only when the bytes are on disk) */}
      {files.map((file, idx) => {
        const downloadable = !!file.key
        const FileIcon = fileIconFor(file.filename, file.mime)
        return (
          <div
            key={idx}
            className={`group mt-2.5 flex w-full items-center rounded-xl bg-black/[0.03] dark:bg-white/[0.03] text-xs font-semibold border border-border/20 ${
              downloadable ? 'hover:bg-black/[0.06] dark:hover:bg-white/[0.06]' : ''
            }`}
          >
            <button
              type="button"
              disabled={!downloadable}
              onClick={() => openAttachment(file)}
              title={downloadable ? t('chat.openFile', { filename: file.filename }) : file.filename}
              className={`flex min-w-0 flex-1 items-center gap-2 rounded-l-xl p-2 text-left ${
                downloadable ? 'cursor-pointer' : 'cursor-default'
              }`}
            >
              <FileIcon size={15} className="text-accent shrink-0" />
              <span className="truncate">{file.filename}</span>
              <span className="text-[0.59375rem] text-secondary ml-auto shrink-0 font-normal">
                {formatFileSize(file.size)}
              </span>
            </button>
            {downloadable && (
              <button
                type="button"
                onClick={() => downloadAttachment(file)}
                title={t('chat.saveFile', { filename: file.filename })}
                aria-label={t('chat.saveFile', { filename: file.filename })}
                className="mr-1 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-secondary opacity-0 transition group-hover:opacity-100 hover:bg-black/[0.06] hover:text-primary dark:hover:bg-white/[0.08] cursor-pointer focus-visible:opacity-100"
              >
                <Download size={13} />
              </button>
            )}
          </div>
        )
      })}
    </>
  )
}
