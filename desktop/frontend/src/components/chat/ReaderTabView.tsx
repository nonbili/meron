import { Code, FileText, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { accounts$ } from '../../states/accounts'
import { closeMessageTab, setTabViewMode } from '../../states/compose'
import { normalizeSenderAddr, settings$ } from '../../states/settings'
import { thread$ } from '../../states/thread'
import type { MessageTab } from '../../types'
import { Composer } from '../composer/Composer'
import { HtmlMessageView } from './HtmlMessageView'
import { AddressRow } from './AddressList'
import { BlockedRemoteButton } from './BlockedRemoteButton'
import { extractAddr, formatFullTimestamp, htmlHasRemoteMedia, isImage, isInlineMedia, isVideo } from './messageHelpers'

// Renders the active reader/compose tab: a compose tab shows the Composer, a
// reader tab shows the message header, address rows and the HTML/plain body.
export function ReaderTabView({ tab }: { tab: MessageTab }) {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  const allowedSenders = useValue(settings$.remoteImageSenders)
  const revealedRemote = useValue(thread$.revealedRemote)
  if (tab.kind === 'compose') {
    return <Composer key={tab.id} tabId={tab.id} />
  }

  // A Reply-To equal to From is what most mail carries and says nothing, so it
  // only earns a row when it points somewhere else — the same rule the bubbles
  // and the mobile reader use.
  const replyToDiffers =
    !!tab.replyTo && extractAddr(tab.replyTo).toLowerCase() !== extractAddr(tab.fromRaw ?? '').toLowerCase()
  const hasAddresses = tab.fromRaw || tab.to || tab.cc || tab.bcc || replyToDiffers

  // The body was baked when the thread was read, so the tab has to re-apply the
  // decision as it stands now: the account toggle, the sender allowlist, or a
  // reveal the user made on this message in the conversation.
  const account = accounts.find((acc) => acc.id === tab.accountId)
  const sender = normalizeSenderAddr(tab.fromRaw ?? '')
  const allowRemote =
    (account?.load_remote_images ?? false) ||
    (!!sender && allowedSenders.includes(sender)) ||
    // `revealMessageRemote` copies a reveal onto the tab as it happens; the
    // live map still counts for a tab opened before this thread was switched.
    !!revealedRemote[tab.messageId] ||
    !!tab.revealRemote
  // The same affordance the conversation offers, on the tab's own header: a tab
  // outlives the thread it was opened from, so it has to be able to reveal the
  // content itself.
  const remoteAttachments = (tab.attachments ?? []).filter(
    (a) => !isInlineMedia(a) && a.url && (isImage(a) || isVideo(a)),
  )
  const hiddenRemoteCount = allowRemote ? 0 : remoteAttachments.length
  const blockedRemote =
    !allowRemote && (hiddenRemoteCount > 0 || (tab.viewMode === 'html' && htmlHasRemoteMedia(tab.bodyHtml)))

  return (
    <>
      <header className="flex h-16 shrink-0 items-center gap-3 border-b border-border bg-header px-4 z-10 select-none">
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-sm font-bold text-primary leading-tight" title={tab.subject}>
            {tab.subject}
          </h2>
          {tab.date && (
            <p className="truncate text-[0.65625rem] text-secondary mt-0.5 font-medium">
              {formatFullTimestamp(tab.date)}
            </p>
          )}
        </div>
        <div className="flex items-center gap-1 rounded-lg bg-hover p-0.5">
          <button
            onClick={() => setTabViewMode(tab.id, 'html')}
            className={`flex items-center gap-1 rounded-md px-2.5 py-1 text-[0.6875rem] font-semibold cursor-pointer ${
              tab.viewMode === 'html' ? 'bg-chats text-accent shadow-sm' : 'text-secondary hover:text-primary'
            }`}
            title={t('chat.htmlView')}
          >
            <Code size={13} /> HTML
          </button>
          <button
            onClick={() => setTabViewMode(tab.id, 'plain')}
            className={`flex items-center gap-1 rounded-md px-2.5 py-1 text-[0.6875rem] font-semibold cursor-pointer ${
              tab.viewMode === 'plain' ? 'bg-chats text-accent shadow-sm' : 'text-secondary hover:text-primary'
            }`}
            title={t('chat.plainView')}
          >
            <FileText size={13} /> {t('settings.account.conversationPlain')}
          </button>
        </div>
        <BlockedRemoteButton
          messageId={tab.messageId}
          blocked={blockedRemote}
          hiddenRemoteCount={hiddenRemoteCount}
          // Trusting the sender of the user's own mail would be a no-op that
          // still grew the allowlist, so it only offers a reveal.
          senderAddress={tab.outgoing ? '' : sender}
          size={16}
        />
        <button
          onClick={() => void closeMessageTab(tab.id)}
          className="flex h-9 w-9 items-center justify-center rounded-full hover:bg-hover text-secondary cursor-pointer"
          title={t('chat.closeTab')}
        >
          <X size={16} />
        </button>
      </header>
      {hasAddresses && (
        <div className="shrink-0 space-y-1 border-b border-border bg-header px-4 py-2.5 text-secondary select-text">
          {tab.fromRaw && <AddressRow label={t('composer.fields.from')} rawList={tab.fromRaw} />}
          {tab.to && <AddressRow label={t('composer.fields.to')} rawList={tab.to} />}
          {tab.cc && <AddressRow label={t('composer.fields.cc')} rawList={tab.cc} />}
          {tab.bcc && <AddressRow label={t('composer.fields.bcc')} rawList={tab.bcc} />}
          {replyToDiffers && <AddressRow label={t('chat.replyTo')} rawList={tab.replyTo!} />}
        </div>
      )}
      <HtmlMessageView
        scrollKey={tab.id}
        title={tab.subject}
        html={tab.bodyHtml}
        text={tab.body}
        attachments={tab.attachments}
        viewMode={tab.viewMode}
        allowRemote={allowRemote}
      />
    </>
  )
}
