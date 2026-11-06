import { ChevronDown, Code, FileText, Forward, MoreHorizontal, Reply, ReplyAll, Trash2 } from 'lucide-react'
import { type ReactNode, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { printMail } from '../../lib/printMail'
import { accounts$ } from '../../states/accounts'
import { closeMessageTab, forwardMessage, patchTabMessage, replyToMessage, setTabViewMode } from '../../states/compose'
import { messageCanReplyAll } from '../../states/composeReply'
import { deleteMessage } from '../../states/mailMoves'
import { mail$ } from '../../states/mailState'
import { normalizeSenderAddr, settings$ } from '../../states/settings'
import { thread$, type MessageContextMenuState } from '../../states/thread'
import type { Account, Message, MessageTab } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { Composer } from '../composer/Composer'
import { HtmlMessageView } from './HtmlMessageView'
import { AddressRow } from './AddressList'
import { BlockedRemoteButton } from './BlockedRemoteButton'
import { CryptoBadge } from '../crypto/CryptoBadge'
import { FrostedBand } from './FrostedBand'
import { MessageContextMenu } from './MessageContextMenu'
import {
  extractAddr,
  formatFullTimestamp,
  formatRecipientSummary,
  htmlHasRemoteMedia,
  isImage,
  isInlineMedia,
  isVideo,
  mediaFilename,
  imageSource,
  parseAddressList,
} from './messageHelpers'

// Renders the active reader/compose tab: a compose tab shows the Composer, a
// reader tab shows the message's subject, addresses and HTML/plain body in one
// scroll, under a floating bar and, with the setting on, floating actions.
export function ReaderTabView({ tab }: { tab: MessageTab }) {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  const allowedSenders = useValue(settings$.remoteImageSenders)
  const revealedRemote = useValue(thread$.revealedRemote)
  const bottomActions = useValue(settings$.readerBottomActions)
  const [menu, setMenu] = useState<ReaderMenuAt | null>(null)
  if (tab.kind === 'compose') {
    return <Composer key={tab.id} tabId={tab.id} />
  }

  // A Reply-To equal to From is what most mail carries and says nothing, so it
  // only earns a row when it points somewhere else — the same rule the bubbles
  // and the mobile reader use.
  const replyToDiffers =
    !!tab.replyTo && extractAddr(tab.replyTo).toLowerCase() !== extractAddr(tab.fromRaw ?? '').toLowerCase()

  // The body was baked when the thread was read, so the tab has to re-apply the
  // decision as it stands now: the account toggle, the sender allowlist, or a
  // reveal the user made on this message in the conversation.
  const account = accounts.find((acc) => acc.id === tab.accountId)
  const isRSS = account?.provider === 'rss' || account?.auth_type === 'rss'
  const sender = normalizeSenderAddr(tab.fromRaw ?? '')
  const encrypted = !!tab.message?.crypto?.encrypted
  const allowRemote =
    // Decrypted mail never loads remote content (audit: no network from
    // decrypted bodies), so the whole allowance is gated on !encrypted.
    !encrypted &&
    ((account?.load_remote_images ?? false) ||
      (!!sender && allowedSenders.includes(sender)) ||
      // `revealMessageRemote` copies a reveal onto the tab as it happens; the
      // session map still counts for a tab opened before the reveal.
      !!revealedRemote[tab.messageId] ||
      !!tab.revealRemote)
  // The same affordance the conversation offers, on the tab's own header: a tab
  // outlives the thread it was opened from, so it has to be able to reveal the
  // content itself.
  const remoteAttachments = (tab.attachments ?? []).filter(
    (a) => !isInlineMedia(a) && a.url && (isImage(a) || isVideo(a)),
  )
  const hiddenRemoteCount = allowRemote ? 0 : remoteAttachments.length
  const blockedRemote =
    !encrypted &&
    !allowRemote &&
    (hiddenRemoteCount > 0 || (tab.viewMode === 'html' && htmlHasRemoteMedia(tab.bodyHtml)))
  const actionBar = bottomActions && !isRSS && !!tab.message

  return (
    <div
      className="relative flex min-h-0 flex-1 flex-col"
      // Right-clicking the message, the email's frame included, opens its menu
      // as it does on a card; on a link, the link's; on a picture, save/copy.
      // A text selection keeps the native menu.
      onContextMenu={(event) => {
        // React bubbles events up from portals too: the image viewer and the
        // menu itself are not the message.
        if (!event.currentTarget.contains(event.target as Node)) return
        if (!tab.message || window.getSelection()?.toString().trim()) return
        // A cached picture: one in the email's frame (forwarded by HtmlFrame)
        // or an attachment's in the grid.
        const imageEl = (event.target as Element | null)?.closest?.('[data-media-key]') as HTMLElement | null
        const imageKey = (event.nativeEvent as { meronImageKey?: string }).meronImageKey ?? imageEl?.dataset.mediaKey
        const img = (event.target as Element | null)?.closest?.('img') as HTMLImageElement | null
        const frameImage =
          (event.nativeEvent as { meronImage?: MessageContextMenuState['image'] }).meronImage ??
          (img ? (imageSource(img.currentSrc || img.src) ?? undefined) : undefined)
        if (!imageKey && !frameImage && (event.target as Element | null)?.closest?.('img')) return
        event.preventDefault()
        const image = imageKey
          ? {
              key: imageKey,
              filename: mediaFilename(imageKey, tab.attachments),
              mime: tab.attachments?.find((att) => att.key === imageKey)?.mime,
            }
          : frameImage
        const anchor = (event.target as Element | null)?.closest?.('a[href]') as HTMLAnchorElement | null
        const linkUrl: string | undefined =
          (event.nativeEvent as { meronLinkUrl?: string }).meronLinkUrl ?? anchor?.href
        setMenu({ x: event.clientX, y: event.clientY, linkUrl, image, tabId: tab.id })
      }}
    >
      <HtmlMessageView
        key={tab.id}
        scrollKey={tab.id}
        title={tab.subject}
        html={tab.bodyHtml}
        text={tab.body}
        attachments={tab.attachments}
        viewMode={tab.viewMode}
        allowRemote={allowRemote}
        article={isRSS}
        // The subject and addresses scroll away with the body, so a long Cc
        // list only costs its height at the top. They start clear of the
        // floating bar, in the body's own column.
        header={
          <div className="max-w-[760px] px-5 pt-16 select-text">
            <h1 className="break-words text-xl font-semibold leading-snug text-primary">
              {tab.subject || '(no subject)'}
            </h1>
            <ReaderCorrespondents
              tab={tab}
              account={account}
              isRSS={isRSS}
              replyToDiffers={replyToDiffers}
              remote={
                <>
                  <CryptoBadge
                    crypto={tab.message?.crypto}
                    accountId={tab.accountId ?? ''}
                    senderName={tab.message?.from_name ?? ''}
                    senderAddress={sender}
                  />
                  <BlockedRemoteButton
                    messageId={tab.messageId}
                    blocked={blockedRemote}
                    hiddenRemoteCount={hiddenRemoteCount}
                    // Trusting the sender of the user's own mail would be a no-op that
                    // still grew the allowlist, so it only offers a reveal.
                    senderAddress={tab.outgoing ? '' : sender}
                  />
                </>
              }
            />
          </div>
        }
        // Room for the body's end to scroll clear of the floating actions.
        footer={actionBar && <div className="h-12" />}
      />
      {/* Frosted under the floating bar, through its bottom edge. */}
      <FrostedBand fade="bottom" className="left-0 right-3 top-0 h-16" />
      <div className="absolute right-5 top-3 z-10 flex items-center gap-1 rounded-full border border-border/40 bg-chats p-1 shadow-float select-none">
        <div className="flex items-center gap-0.5 rounded-full bg-hover p-0.5">
          <button
            onClick={() => setTabViewMode(tab.id, 'html')}
            className={`flex h-7 items-center gap-1 rounded-full px-2.5 text-[0.6875rem] font-semibold cursor-pointer ${
              tab.viewMode === 'html' ? 'bg-chats text-accent shadow-sm' : 'text-secondary hover:text-primary'
            }`}
            title={t('chat.htmlView')}
          >
            <Code size={13} /> HTML
          </button>
          <button
            onClick={() => setTabViewMode(tab.id, 'plain')}
            className={`flex h-7 items-center gap-1 rounded-full px-2.5 text-[0.6875rem] font-semibold cursor-pointer ${
              tab.viewMode === 'plain' ? 'bg-chats text-accent shadow-sm' : 'text-secondary hover:text-primary'
            }`}
            title={t('chat.plainView')}
          >
            <FileText size={13} /> {t('settings.account.conversationPlain')}
          </button>
        </div>
        {tab.message && (
          <button
            type="button"
            onClick={(event) => {
              const rect = event.currentTarget.getBoundingClientRect()
              setMenu({ x: rect.right, y: rect.bottom + 4, tabId: tab.id })
            }}
            className="flex h-8 w-8 items-center justify-center rounded-full text-secondary cursor-pointer hover:bg-hover hover:text-primary"
            title={t('common.more')}
            aria-label={t('chat.moreMessageActions')}
          >
            <MoreHorizontal size={16} />
          </button>
        )}
      </div>
      {actionBar && (
        <div className="pointer-events-none absolute inset-x-0 bottom-0 z-10">
          {/* From a line the bar covers, as under the quick reply. */}
          <FrostedBand fade="top" className="bottom-0 left-0 right-3 top-5" />
          <div className="relative px-3 pb-3 pt-1">
            <ReaderActionBar tabId={tab.id} message={tab.message!} bodyMissing={!!tab.bodyMissing} />
          </div>
        </div>
      )}
      {/* The view is reused for the next tab when this one closes; a menu
          opened here must not act on that tab's message. */}
      {menu?.tabId === tab.id && tab.message && (
        <ReaderMessageMenu
          at={menu}
          tab={tab}
          message={tab.message}
          isRSS={isRSS}
          allowRemote={allowRemote}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  )
}

// Who sent the message and when, folded to the sender and a "to …" line like
// the conversation's message headers: the full lists are rarely what the
// reader came for. The chevron unfolds every list, each pill copying its full
// address.
function ReaderCorrespondents({
  tab,
  account,
  isRSS,
  replyToDiffers,
  remote,
}: {
  tab: MessageTab
  account: Account | undefined
  isRSS: boolean
  replyToDiffers: boolean
  /** The remote-content affordance, beside the date. */
  remote: ReactNode
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const from = parseAddressList(tab.fromRaw)[0]
  const senderName = from?.name || tab.from
  const senderAddr = from?.email || tab.fromAddr
  const outgoing = !!tab.outgoing
  const recipients = formatRecipientSummary(tab.to, tab.cc)
  // A feed item has no recipients, so the lists could only repeat the feed
  // name the header already shows.
  const canOpen = !isRSS && !!(tab.to || tab.cc || tab.bcc || replyToDiffers)
  const stamp = tab.date ? formatFullTimestamp(tab.date) : ''

  return (
    <div className="mt-3 rounded-xl border border-border/40 bg-chats px-4 py-3 shadow-card">
      <div className="flex items-start gap-2.5">
        <Avatar
          name={(outgoing ? account?.display_name : '') || senderName}
          email={outgoing ? undefined : senderAddr}
          src={outgoing ? account?.avatar_url : tab.feedIcon ? `/media/${tab.feedIcon}` : undefined}
          size={32}
          className="mt-0.5 shrink-0"
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-1.5">
            <span className="truncate text-[0.84375rem] font-semibold text-primary">{senderName}</span>
            {senderAddr && senderAddr !== senderName && (
              <span className="truncate text-[0.71875rem] text-secondary/80">{senderAddr}</span>
            )}
          </div>
          {(recipients || canOpen) && (
            <div className="flex min-w-0 items-center gap-1">
              {recipients && (
                <span
                  className="truncate text-[0.71875rem] text-secondary/80"
                  title={[tab.to, tab.cc].filter(Boolean).join(', ')}
                >
                  {t('chat.toRecipients', { recipients })}
                </span>
              )}
              {canOpen && (
                <button
                  type="button"
                  onClick={() => setOpen((value) => !value)}
                  title={open ? t('chat.hideDetails') : t('chat.showDetails')}
                  aria-expanded={open}
                  className="flex h-4 w-4 shrink-0 items-center justify-center rounded text-secondary hover:bg-black/[0.05] hover:text-primary dark:hover:bg-white/[0.08] cursor-pointer transition-colors select-none"
                >
                  <ChevronDown size={12} className={`transition-transform ${open ? 'rotate-180' : ''}`} />
                </button>
              )}
            </div>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1.5 text-[0.65625rem] text-secondary/80 select-none">
          {remote}
          {stamp && <span>{stamp}</span>}
        </div>
      </div>
      {open && (
        <div className="mt-2.5 space-y-1 border-t border-border/40 pt-2.5 text-secondary">
          {tab.fromRaw && <AddressRow label={t('composer.fields.from')} rawList={tab.fromRaw} />}
          {tab.to && <AddressRow label={t('composer.fields.to')} rawList={tab.to} />}
          {tab.cc && <AddressRow label={t('composer.fields.cc')} rawList={tab.cc} />}
          {tab.bcc && <AddressRow label={t('composer.fields.bcc')} rawList={tab.bcc} />}
          {replyToDiffers && <AddressRow label={t('chat.replyTo')} rawList={tab.replyTo!} />}
        </div>
      )}
    </div>
  )
}

type ReaderMenuAt = {
  x: number
  y: number
  linkUrl?: string
  image?: MessageContextMenuState['image']
  tabId: string
}

// The message card's menu, on the tab. It reads the message from the open
// thread while that is loaded, so a star or read change made anywhere shows;
// otherwise from the tab's own copy, which its changes are written back to.
function ReaderMessageMenu({
  at,
  tab,
  message,
  isRSS,
  allowRemote,
  onClose,
}: {
  at: ReaderMenuAt
  tab: MessageTab
  message: Message
  isRSS: boolean
  allowRemote: boolean
  onClose: () => void
}) {
  const live = useValue(() => mail$.messages.get().find((item) => item.id === message.id))
  return (
    <MessageContextMenu
      state={{ ...at, message: live ?? message, hideOpenInNewTab: true }}
      isRSS={isRSS}
      // Until the body arrives, forwarding or copying it would take an empty one.
      headerOnly={!!tab.bodyMissing}
      onClose={onClose}
      onMessageChange={(patch) => patchTabMessage(tab.id, patch)}
      onReply={() => replyToMessage(live ?? message)}
      // The tab's own body, with the remote content it shows right now.
      onPrint={() => void printMail(tab, allowRemote)}
      // The tab would otherwise go on showing a message that is gone.
      onDeleted={() => void closeMessageTab(tab.id)}
    />
  )
}

// Reply, forward and delete floating at the foot of the reader, where they are easier to
// reach than the conversation's menus (the reader-bottom-actions setting).
function ReaderActionBar({
  tabId,
  message,
  bodyMissing,
}: {
  tabId: string
  message: Message
  /** Forwarding needs the body and its attachments, so it waits for them. */
  bodyMissing: boolean
}) {
  const { t } = useTranslation()
  return (
    <footer className="pointer-events-auto flex items-center justify-center gap-2 rounded-2xl border border-border/40 bg-chats p-1.5 shadow-float select-none">
      <ReaderAction
        icon={<Reply size={15} />}
        label={t('chat.actions.reply')}
        onClick={() => replyToMessage(message)}
      />
      {messageCanReplyAll(message) && (
        <ReaderAction
          icon={<ReplyAll size={15} />}
          label={t('chat.actions.replyAll')}
          onClick={() => replyToMessage(message, true)}
        />
      )}
      <ReaderAction
        icon={<Forward size={15} />}
        label={t('chat.actions.forward')}
        disabled={bodyMissing}
        onClick={() => void forwardMessage(message)}
      />
      <ReaderAction
        icon={<Trash2 size={15} />}
        label={t('buttons.delete')}
        danger
        onClick={async () => {
          // The tab would otherwise go on showing a message that is gone.
          if (await deleteMessage(message)) void closeMessageTab(tabId)
        }}
      />
    </footer>
  )
}

function ReaderAction({
  icon,
  label,
  danger = false,
  disabled = false,
  onClick,
}: {
  icon: ReactNode
  label: string
  danger?: boolean
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-semibold cursor-pointer hover:bg-hover disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent ${
        danger ? 'text-red-500' : 'text-secondary hover:text-primary'
      }`}
    >
      {icon}
      {label}
    </button>
  )
}
