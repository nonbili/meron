import { useState } from 'react'
import type { MouseEvent } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import {
  AlertCircle,
  Check,
  ChevronDown,
  Download,
  ExternalLink,
  Image,
  Loader2,
  MoreHorizontal,
  Star,
} from 'lucide-react'

import { openExternal } from '../../lib/native'
import { downloadAttachment, getActiveThread, isDraftFolder } from '../../states/mail'
import { openDraftCompose, openMessageTab, retrySend } from '../../states/compose'
import { accountIdentities, accounts$ } from '../../states/accounts'
import type { Message } from '../../types'
import { revealRemote, thread$ } from '../../states/thread'
import {
  extractAddr,
  fileIconFor,
  formatFileSize,
  formatFullTimestamp,
  formatMessageStamp,
  getVisibleMedia,
  htmlReferencesMedia,
  mediaSrc,
  parseAddressList,
} from './messageHelpers'
import { AddressRow } from './AddressList'
import { MessageBubbleBody } from './MessageBubbleBody'
import { VideoAttachment } from './VideoAttachment'
import type { MessageContextMenuState } from './MessageContextMenu'

interface MessageBubbleProps {
  message: Message
  // Index of this bubble's first image within the thread-wide gallery list.
  galleryOffset: number
  onOpenContextMenu: (state: MessageContextMenuState) => void
  onLinkHover?: (url: string | null) => void
}

export function MessageBubble({ message, galleryOffset, onOpenContextMenu, onLinkHover }: MessageBubbleProps) {
  const { t } = useTranslation()
  const [metaOpen, setMetaOpen] = useState(false)
  const accounts = useValue(accounts$)
  const search = useValue(thread$.search)
  const activeSearchId = useValue(thread$.activeSearchId)
  const activeThread = useValue(getActiveThread)
  const revealedMap = useValue(thread$.revealedRemote)
  const revealed = !!revealedMap[message.id]
  const modeOverrides = useValue(thread$.conversationModeOverrides)

  const account = accounts.find((acc) => acc.id === message.account_id)
  const fromEmail = message.from_addr.trim().toLowerCase()
  const outgoing =
    !!message.send_status ||
    message.outgoing === true ||
    (!!account && accountIdentities(account).some((identity) => identity.email.trim().toLowerCase() === fromEmail))
  const isDraft = isDraftFolder(message.folder_id, message.account_id)
  const activeAccount = activeThread ? accounts.find((acc) => acc.id === activeThread.account_id) : null
  const isRSS = activeAccount?.provider === 'rss' || activeAccount?.auth_type === 'rss'
  const showOriginalDate = isRSS
  const accountConversationMode = (activeAccount?.conversation_html ?? true) ? 'html' : 'plain'
  const conversationMode = activeAccount ? (modeOverrides[activeAccount.id] ?? accountConversationMode) : 'plain'

  const { attachmentImages, videos, hiddenRemoteCount, files } = getVisibleMedia(message, account, revealed)
  const normalizedSearchQuery = search.trim()
  const useHtmlBody = conversationMode === 'html' && !!message.body_html && !normalizedSearchQuery
  const showAttachmentImages =
    attachmentImages.length > 0 &&
    (!useHtmlBody || (outgoing && attachmentImages.some((image) => !htmlReferencesMedia(message.body_html, image))))
  const activeSearchMatch = activeSearchId === message.id

  const replyToRaw = message.reply_to?.trim()
  const ccRaw = message.cc?.trim()
  const toRaw = message.to?.trim()
  const replyToDiffers =
    !outgoing && !!replyToRaw && extractAddr(replyToRaw).toLowerCase() !== message.from_addr.toLowerCase()
  // For incoming mail, hide To only when this account is the sole visible
  // recipient. If there are other To recipients, show the whole list.
  const accountEmails = new Set(
    account
      ? accountIdentities(account)
          .map((identity) => identity.email.trim().toLowerCase())
          .filter(Boolean)
      : [],
  )
  const toRecipients = parseAddressList(toRaw ?? '')
  const showTo =
    !outgoing &&
    !!toRaw &&
    toRecipients.length > 0 &&
    (accountEmails.size === 0 || toRecipients.some((a) => !accountEmails.has(a.email.toLowerCase())))
  const hasMeta = !outgoing && !!(replyToDiffers || ccRaw || showTo)

  const onOpenImage = (idx: number) => thread$.galleryIndex.set(idx)
  const openActionsMenu = (event: MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    onOpenContextMenu({ x: rect.right, y: rect.bottom + 4, message, hideOpenInNewTab: true })
  }
  const openMessageOrDraftTab = () => {
    if (isDraft) {
      void openDraftCompose(message)
      return
    }
    openMessageTab(message)
  }

  return (
    <div className={`flex w-full animate-slide-up ${outgoing ? 'justify-end' : 'justify-start'}`}>
      <div
        className={`group/message-bubble relative ${useHtmlBody ? 'w-[70%]' : 'max-w-[70%]'} min-w-[100px] p-3.5 border transition-shadow duration-200 ${
          isDraft
            ? 'bg-bubble-out/55 text-bubble-out-text/80 border-dashed border-accent/45 rounded-2xl rounded-tr-sm shadow-none'
            : outgoing
              ? 'bg-bubble-out text-bubble-out-text border-border/35 rounded-2xl rounded-tr-sm shadow-bubble-out'
              : 'bg-bubble-in text-bubble-in-text border-border/40 rounded-2xl rounded-tl-sm shadow-bubble-in'
        }`}
      >
        <div className="absolute right-2 -top-3.5 z-20 flex items-center gap-1 rounded-full border border-border/40 bg-header/95 p-0.5 text-secondary opacity-0 shadow-sm transition-opacity group-hover/message-bubble:opacity-100 focus-within:opacity-100">
          <button
            type="button"
            title={isDraft ? t('chat.actions.openDraft') : t('threads.actions.openInNewTab')}
            aria-label={isDraft ? t('chat.actions.openDraft') : t('threads.actions.openInNewTab')}
            onClick={openMessageOrDraftTab}
            className="flex h-6 w-6 items-center justify-center rounded-full hover:bg-hover hover:text-primary cursor-pointer transition-colors"
          >
            <ExternalLink size={13} />
          </button>
          <button
            type="button"
            title={t('common.more')}
            aria-label={t('chat.moreMessageActions')}
            onClick={openActionsMenu}
            className="flex h-6 w-6 items-center justify-center rounded-full hover:bg-hover hover:text-primary cursor-pointer transition-colors"
          >
            <MoreHorizontal size={14} />
          </button>
        </div>

        {/* Header: sender + optional meta toggle on the left, timestamp on the right */}
        <div className="flex items-center justify-between gap-2 mb-1.5">
          <div className="relative flex items-center gap-1 min-w-0">
            {!outgoing && (
              <span className="text-[12.5px] font-bold text-accent select-none truncate tracking-wide">
                {message.from_name || message.from_addr}
              </span>
            )}
            {hasMeta && (
              <button
                type="button"
                onClick={() => setMetaOpen((open) => !open)}
                title={metaOpen ? t('chat.hideDetails') : t('chat.showDetails')}
                className="flex items-center justify-center w-4 h-4 rounded text-secondary hover:text-primary hover:bg-black/[0.05] dark:hover:bg-white/[0.08] cursor-pointer transition-colors"
              >
                <ChevronDown size={12} className={`transition-transform ${metaOpen ? 'rotate-180' : ''}`} />
              </button>
            )}
            {hasMeta && metaOpen && (
              <>
                <div className="fixed inset-0 z-40" onClick={() => setMetaOpen(false)} />
                <div className="absolute left-0 top-full mt-1 z-50 w-[460px] max-w-[calc(100vw-48px)] max-h-[260px] overflow-y-auto space-y-2 rounded-lg border border-border bg-chats p-3 shadow-xl text-secondary select-text">
                  {showTo && <AddressRow label={t('composer.fields.to')} rawList={toRaw!} />}
                  {replyToDiffers && <AddressRow label="Reply-To" rawList={replyToRaw!} />}
                  {ccRaw && <AddressRow label={t('composer.fields.cc')} rawList={ccRaw} />}
                </div>
              </>
            )}
          </div>
          <div className="flex items-center gap-1 text-[10.5px] text-secondary/80 select-none shrink-0">
            {isDraft && (
              <span className="rounded-full border border-accent/35 bg-accent/10 px-1.5 py-0.5 text-[9.5px] font-bold uppercase tracking-wide text-accent">
                {t('chat.draft')}
              </span>
            )}
            {message.starred && <Star size={11} className="fill-amber-500 text-amber-500" />}
            <span title={formatFullTimestamp(message.date)}>{formatMessageStamp(message.date, showOriginalDate)}</span>
            {outgoing &&
              !isDraft &&
              (message.send_status === 'sending' ? (
                <Loader2 size={12} className="text-secondary/70 animate-spin" />
              ) : message.send_status === 'failed' ? (
                <button
                  type="button"
                  title={t('chat.failedToSendRetry')}
                  onClick={() => void retrySend(message.id)}
                  className="flex items-center gap-0.5 text-red-500 hover:text-red-600 cursor-pointer"
                >
                  <AlertCircle size={12} />
                  <span className="text-[10px] font-semibold">{t('chat.retry')}</span>
                </button>
              ) : (
                <Check size={12} className="text-accent opacity-90" />
              ))}
          </div>
        </div>

        {/* Image attachments */}
        {showAttachmentImages &&
          (() => {
            const count = attachmentImages.length
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
                {attachmentImages.map((image, idx) => {
                  const src = mediaSrc(image)
                  return (
                    <button
                      key={idx}
                      type="button"
                      onClick={() => onOpenImage(galleryOffset + idx)}
                      className={`block w-full overflow-hidden hover:opacity-90 cursor-pointer ${btnClass}`}
                      title={image.filename}
                    >
                      <img
                        src={src}
                        alt={image.filename}
                        loading="lazy"
                        className="w-full h-full object-cover object-top"
                      />
                    </button>
                  )
                })}
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
            className="mb-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed border-border/50 bg-black/[0.02] dark:bg-white/[0.02] py-2 text-[11px] font-semibold text-secondary hover:text-accent hover:border-accent/40 cursor-pointer transition-colors"
          >
            <Image size={13} />
            {t('chat.showImages', { count: hiddenRemoteCount })}
          </button>
        )}

        <MessageBubbleBody
          message={message}
          useHtmlBody={useHtmlBody}
          normalizedSearchQuery={normalizedSearchQuery}
          activeSearchMatch={activeSearchMatch}
          onLinkHover={onLinkHover}
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
              <span className="text-[9.5px] text-secondary ml-auto shrink-0 font-normal">
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
      </div>
    </div>
  )
}
