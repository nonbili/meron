import { useValue } from '@legendapp/state/react'
import { accountIdentities, accounts$ } from '../../states/accounts'
import { getActiveThread, isDraftFolder } from '../../states/mail'
import { thread$ } from '../../states/thread'
import type { Account, Message } from '../../types'
import { extractAddr, formatRecipientSummary, getVisibleMedia, htmlReferencesMedia } from './messageHelpers'

export type MessageView = ReturnType<typeof useMessageView>

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
  const activeThread = useValue(getActiveThread)
  const revealedMap = useValue(thread$.revealedRemote)
  const modeOverrides = useValue(thread$.conversationModeOverrides)

  const account: Account | undefined = accounts.find((acc) => acc.id === message.account_id)
  const fromEmail = message.from_addr.trim().toLowerCase()
  const outgoing =
    !!message.send_status ||
    message.outgoing === true ||
    (!!account && accountIdentities(account).some((identity) => identity.email.trim().toLowerCase() === fromEmail))
  const isDraft = isDraftFolder(message.folder_id, message.account_id)
  const activeAccount = activeThread ? accounts.find((acc) => acc.id === activeThread.account_id) : null
  const isRSS = activeAccount?.provider === 'rss' || activeAccount?.auth_type === 'rss'
  const accountConversationMode = (activeAccount?.conversation_html ?? true) ? 'html' : 'plain'
  const conversationMode = activeAccount ? (modeOverrides[activeAccount.id] ?? accountConversationMode) : 'plain'

  const revealed = !!revealedMap[message.id]
  const { attachmentImages, videos, hiddenRemoteCount, files } = getVisibleMedia(message, account, revealed)
  const normalizedSearchQuery = search.trim()
  const useHtmlBody = conversationMode === 'html' && !!message.body_html && !normalizedSearchQuery
  const showAttachmentImages =
    attachmentImages.length > 0 &&
    (!useHtmlBody || (outgoing && attachmentImages.some((image) => !htmlReferencesMedia(message.body_html, image))))

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
    attachmentImages,
    videos,
    hiddenRemoteCount,
    files,
    showAttachmentImages,
    useHtmlBody,
    normalizedSearchQuery,
    activeSearchMatch: activeSearchId === message.id,
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
