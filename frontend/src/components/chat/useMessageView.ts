import { useValue } from '@legendapp/state/react'
import { accountIdentities, accounts$ } from '../../states/accounts'
import { getActiveThread, isDraftFolder } from '../../states/mail'
import { settings$ } from '../../states/settings'
import { thread$ } from '../../states/thread'
import type { ConversationMode } from '../../states/thread'
import type { Account, Message } from '../../types'
import {
  extractAddr,
  formatRecipientSummary,
  getVisibleMedia,
  htmlHasRemoteMedia,
  remoteContentAllowed,
  standaloneAttachmentImages,
} from './messageHelpers'

export type MessageView = ReturnType<typeof useMessageView>

/** The render mode the open thread's account is on: the session override when
 *  the user flipped it in the header, otherwise the account setting. */
export function useConversationMode(): ConversationMode {
  const accounts = useValue(accounts$)
  const activeThread = useValue(getActiveThread)
  const modeOverrides = useValue(thread$.conversationModeOverrides)
  const activeAccount = activeThread ? accounts.find((acc) => acc.id === activeThread.account_id) : null
  if (!activeAccount) return 'plain'
  return modeOverrides[activeAccount.id] ?? ((activeAccount.conversation_html ?? true) ? 'html' : 'plain')
}

/**
 * Everything both conversation layouts derive from a message: who sent it, which
 * body renderer applies, and the media split. MessageBubble (chat) and
 * MessageRow (traditional) each call this and hand the result to MessageContent,
 * so the two layouts differ only in chrome.
 */
export function useMessageView(message: Message) {
  const accounts = useValue(accounts$)
  const search = useValue(thread$.search)
  const activeSearchId = useValue(thread$.activeSearchId)
  const activeSearchOffset = useValue(thread$.activeSearchOffset)
  const activeThread = useValue(getActiveThread)
  const revealedMap = useValue(thread$.revealedRemote)
  const conversationMode = useConversationMode()
  const allowedSenders = useValue(settings$.remoteImageSenders)

  const account: Account | undefined = accounts.find((acc) => acc.id === message.account_id)
  const fromEmail = message.from_addr.trim().toLowerCase()
  const outgoing =
    !!message.send_status ||
    message.outgoing === true ||
    (!!account && accountIdentities(account).some((identity) => identity.email.trim().toLowerCase() === fromEmail))
  const isDraft = isDraftFolder(message.folder_id, message.account_id)
  const activeAccount = activeThread ? accounts.find((acc) => acc.id === activeThread.account_id) : null
  const isRSS = activeAccount?.provider === 'rss' || activeAccount?.auth_type === 'rss'

  const revealed = !!revealedMap[message.id]
  const { attachmentImages, videos, hiddenRemoteCount, files } = getVisibleMedia(
    message,
    account,
    revealed,
    allowedSenders,
  )
  const remoteVisible = remoteContentAllowed(message, account, allowedSenders) || revealed
  const normalizedSearchQuery = search.trim()
  // A search must not change how a message reads: HTML bodies stay HTML and
  // highlight their matches inside the frame (BubbleHtmlFrame), rather than
  // falling back to the plain-text renderer for the duration of the search.
  const useHtmlBody = conversationMode === 'html' && !!message.body_html
  // A newsletter's remote images live in the HTML body, not in the attachment
  // list, so the reveal affordance has to look at the body too — otherwise the
  // most common blocked message offers no way to show its content.
  const blockedRemote =
    !remoteVisible && (hiddenRemoteCount > 0 || (useHtmlBody && htmlHasRemoteMedia(message.body_html)))
  const bubbleAttachmentImages = standaloneAttachmentImages(attachmentImages, useHtmlBody, message.body_html)

  const replyToRaw = message.reply_to?.trim()
  const ccRaw = message.cc?.trim()
  const toRaw = message.to?.trim()
  const bccRaw = message.bcc?.trim()
  const replyToDiffers =
    !outgoing && !!replyToRaw && extractAddr(replyToRaw).toLowerCase() !== message.from_addr.toLowerCase()
  const fromRaw = message.from_name ? `${message.from_name} <${message.from_addr}>` : message.from_addr

  return {
    account,
    outgoing,
    isDraft,
    isRSS,
    /** RSS items show their published date rather than a relative stamp. */
    showOriginalDate: isRSS,
    revealed,
    /** Whether this message's remote content is currently allowed to load. */
    remoteVisible,
    /** Whether remote content is being held back and can be revealed. */
    blockedRemote,
    /** Bare From address, the key the sender allowlist is stored under. */
    senderAddress: fromEmail,
    attachmentImages,
    bubbleAttachmentImages,
    videos,
    hiddenRemoteCount,
    files,
    useHtmlBody,
    normalizedSearchQuery,
    activeSearchMatch: activeSearchId === message.id,
    /** Which occurrence inside this message the search is parked on, -1 when
     *  it's parked on another message (or on a subject-only match). */
    activeSearchOffset: activeSearchId === message.id ? activeSearchOffset : -1,
    // Outgoing messages have no sender name worth showing, and without
    // recipients a reply and a forward of the same text are indistinguishable —
    // so recipients take the sender slot, the way Gmail's "to …" line does.
    recipientSummary: outgoing ? formatRecipientSummary(toRaw, ccRaw) : '',
    /** The same summary regardless of direction, for layouts that always show a
     *  "to …" line. */
    allRecipientSummary: formatRecipientSummary(toRaw, ccRaw),
    // An outgoing message is us: show the account's own avatar — the image the
    // side navigation shows — instead of resolving the From address.
    avatarSrc: outgoing ? account?.avatar_url : undefined,
    avatarName: (outgoing ? account?.display_name : '') || message.from_name || message.from_addr,
    avatarEmail: (outgoing ? account?.email : '') || message.from_addr,
    fromRaw,
    toRaw,
    ccRaw,
    bccRaw,
    replyToRaw,
    replyToDiffers,
  }
}
