import { threadWindowSession } from '../lib/windowSession'
import {
  standaloneCompose,
  STANDALONE_COMPOSE_KEY,
  externalComposeDraft$,
  forgetStandaloneRecoveryDraft,
} from '../lib/composeWindow'
import { t } from '../lib/i18n'
import type { Attachment, ComposeDraft, Message } from '../types'
import { invoke } from '../lib/bridge'
import { CONVERSATION_PAGE_SIZE } from '../lib/pagination'
import { ui$, showToast } from './ui'
import { withoutInlineRichStyles } from '../components/composer/composerHelpers'
import { accounts$, isSendableAccount } from './accounts'
import { normalizeMessageId } from './mail'
import { mail$ } from './mailState'
import { isDraftFolder } from './mailFolders'
import { decryptFailed } from '../lib/crypto'
import { draftSavesSettled, getComposeSession } from './composeSessions'
import { loadPersistedTabs, compose$, hasExtraComposeHeaders, newDraftMessageId } from './composeState'
import { discardingDraftIds, reserveOpeningDraft } from './quickReply'
import { ComposeSeed, openComposeTab, readComposerAttachments } from './compose'
import { foldDraftQuote } from './composeQuote'
import { newestMessage, updateComposeDraft } from './messageTabs'

// Fill a tab just opened with a message's attachments. Until they land the
// session holds the fetch, so an Esc save can wait for them: it would otherwise
// keep the message without its attachments, and a saved draft replaced with
// that copy would lose them for good. The tab may have been edited or closed
// meanwhile; bail if it's gone, otherwise merge onto what it now holds.
export async function fetchComposeTabAttachments(id: string, attachments: Attachment[], inlineHtml: string) {
  const session = getComposeSession(id)
  const fetch = readComposerAttachments(attachments, inlineHtml)
  session.attachmentFetches.add(fetch)
  const valid = await fetch.finally(() => session.attachmentFetches.delete(fetch))
  if (valid.length === 0) return
  const tab = compose$.tabs.get().find((t) => t.id === id)
  if (!tab?.compose) return
  updateComposeDraft(id, { attachments: [...tab.compose.attachments, ...valid] })
}

function composeFromDraftMessage(message: Message): ComposeSeed {
  const body = foldDraftQuote(message)
  return {
    accountId: message.account_id || undefined,
    fromEmail: message.from_addr ?? '',
    to: message.to ?? '',
    cc: message.cc ?? '',
    bcc: message.bcc ?? '',
    replyTo: message.reply_to ?? '',
    showCcBcc: hasExtraComposeHeaders({ cc: message.cc ?? '', bcc: message.bcc ?? '' }),
    subject: message.subject ?? '',
    rich: body.rich,
    // As the editor wrote it, without the styles a save adds: a signature
    // holding a link is recognised (and tracked) only in that form.
    html: withoutInlineRichStyles(body.html),
    text: body.text,
    quote: body.quote && { rich: body.rich, ...body.quote },
    inReplyTo: '',
    references: message.references ?? '',
    // Not an id already being expunged: that copy is going away, so this tab's
    // content belongs in a draft of its own.
    draftMessageId:
      message.message_id && !discardingDraftIds.has(normalizeMessageId(message.message_id))
        ? message.message_id
        : newDraftMessageId(),
    sourceDraft: {
      threadId: message.thread_id,
      messageId: message.id,
      folderId: message.folder_id,
    },
    title: message.subject || 'New message',
    noSignature: true,
    // An encrypted draft reopens encrypted, and its quote (if any) came from
    // an encrypted conversation, so it stays guarded the same way.
    // A plain draft carrying a sign switch was switched off on purpose (the
    // switch is only set with encryption off): it stays off.
    encrypt: message.crypto?.encrypted ? true : message.draft_sign != null ? false : undefined,
    encryptOffByUser: (!message.crypto?.encrypted && message.draft_sign != null) || undefined,
    // The sign switch as it stood when the draft was saved.
    sign: message.draft_sign ?? undefined,
    sensitiveSource: message.crypto?.encrypted && body.quote ? 'reply' : undefined,
    // Its body may hold decrypted text (a quote unfolded and answered inline,
    // a forward): it stays encrypted unless switched off on purpose.
    encryptedDraft: message.crypto?.encrypted || undefined,
  }
}

/** Whether a compose draft is the one this Drafts row holds a copy of. */
function composeHoldsDraft(compose: ComposeDraft, draft: Message): boolean {
  const source = compose.sourceDraft
  return (
    (!!draft.message_id && normalizeMessageId(compose.draftMessageId) === normalizeMessageId(draft.message_id)) ||
    source?.messageId === draft.id ||
    (source?.threadId === draft.thread_id && source?.folderId === draft.folder_id)
  )
}

function activateOpenDraftCompose(draft: Message): boolean {
  const existing = compose$.tabs
    .get()
    .find((tab) => tab.kind === 'compose' && !!tab.compose && composeHoldsDraft(tab.compose, draft))
  if (!existing) return false
  compose$.activeTab.set(existing.id)
  return true
}

/**
 * Before opening a Drafts row, let this draft's Esc save finish. The row can be
 * the copy that save is replacing, and a tab opened from it would autosave the
 * older text over the newer. The tab comes back as it closed, as the store may
 * not hold the new copy yet; when the save failed, the tab is already back.
 * True when a tab now holds the draft.
 */
async function reopenAfterDraftSaves(draft: Message): Promise<boolean> {
  const [saved] = await draftSavesSettled((compose) => composeHoldsDraft(compose, draft))
  // An auxiliary editor released its ownership when that save closed the tab.
  // Route its later reopen through the main window's ownership checks.
  if (threadWindowSession) return activateOpenDraftCompose(draft)
  saved?.reopen()
  return !!saved || activateOpenDraftCompose(draft)
}

export function draftShouldOpenConversation(messages: Message[], draft: Message): boolean {
  return (
    messages.some((message) => !isDraftFolder(message.folder_id, message.account_id)) ||
    !!draft.references?.trim() ||
    !!draft.original_thread_id?.trim()
  )
}

function focusStandaloneDraft(draft: Message): Promise<boolean> | null {
  if (
    standaloneCompose ||
    !draft.message_id ||
    normalizeMessageId(externalComposeDraft$.peek()) !== normalizeMessageId(draft.message_id)
  )
    return null
  return invoke<boolean>('composer.focusDraft', { draft_id: draft.message_id }).then((focused) => focused === true)
}

async function openDraftMessageInCompose(draft: Message) {
  if (activateOpenDraftCompose(draft)) return true
  if (
    (await invoke<boolean>('threadWindow.focusDraft', { draft_id: draft.message_id || draft.id }).catch(
      () => false,
    )) === true
  )
    return true
  if (threadWindowSession) {
    await invoke('threadWindow.openDraft', { draft })
    return true
  }
  const focus = focusStandaloneDraft(draft)
  if (focus && (await focus)) return true
  if (!standaloneCompose) {
    const recovery = loadPersistedTabs(STANDALONE_COMPOSE_KEY).find(
      (tab) => tab.compose?.accountId === draft.account_id && composeHoldsDraft(tab.compose, draft),
    )?.compose
    if (recovery && (await invoke<boolean>('composer.isOpen')) !== true) {
      // The recovery buffer can contain edits newer than the server copy. Move
      // those into the inbox editor before retiring the hidden tray snapshot.
      const id = openComposeTab({
        ...recovery,
        noSignature: true,
        quote: recovery.quote && { rich: recovery.rich, ...recovery.quote },
        sourceDraft: { threadId: draft.thread_id, messageId: draft.id, folderId: draft.folder_id },
      })
      if (id) {
        forgetStandaloneRecoveryDraft(recovery.draftMessageId, recovery.accountId)
        await fetchComposeTabAttachments(id, draft.attachments ?? [], recovery.html)
      }
      return true
    }
  }
  // A draft whose body couldn't be fetched or decrypted would seed an empty
  // editor holding the real draft's id: a save would overwrite it with nothing,
  // ✕ discard it unseen.
  if (draft.body_missing || decryptFailed(draft.crypto)) {
    showDraftNotFound()
    return true
  }
  const id = openComposeTab(composeFromDraftMessage(draft))
  if (!id) return true

  await fetchComposeTabAttachments(id, draft.attachments ?? [], draft.body_html ?? '')
  return true
}

// The read found no draft behind a Drafts row (a stale card, or the draft is
// gone). The row is only a list summary: its From is the counterpart and an
// encrypted draft's subject is the placeholder. A composer seeded from it would
// still carry the real draft's id, so ✕ would discard that draft and every save
// would be refused or overwrite it with an empty copy.
function showDraftNotFound() {
  showToast(t('compose.toast.couldNotOpenDraft'), 'error')
}

// Restore a saved server-side Drafts row into the full composer. Drafts are
// stored as normal IMAP messages, but clicking one should resume editing rather
// than open a read-only conversation.
export async function openDraftCompose(thread: Message) {
  if (!isDraftFolder(thread.folder_id, thread.account_id)) return false
  if (accounts$.get().filter(isSendableAccount).length === 0) {
    showToast(t('compose.toast.addMailAccountBeforeComposing'))
    return true
  }

  if (activateOpenDraftCompose(thread)) return true

  const release = reserveOpeningDraft(thread)
  try {
    const focus = focusStandaloneDraft(thread)
    if (focus && (await focus)) return true
    if (await reopenAfterDraftSaves(thread)) return true
    const result = await invoke<{ messages: Message[] }>('mail.threadRead', {
      thread_id: thread.thread_id,
      limit: CONVERSATION_PAGE_SIZE,
    })
    const draft = newestMessage(
      (result.messages ?? []).filter((message) => isDraftFolder(message.folder_id, message.account_id)),
    )
    if (!draft) {
      showDraftNotFound()
      return true
    }
    // The read can resolve to a copy under a different id; hold that one too for
    // the (synchronous) hop to the tab that now owns it.
    const releaseResolved = reserveOpeningDraft(draft)
    try {
      await openDraftMessageInCompose(draft)
    } finally {
      releaseResolved()
    }
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenDraft'), 'error')
  } finally {
    release()
  }
  return true
}

// A draft clicked in another webview can already belong to the main window's
// quick reply, including newer unsaved text. Focus that owner without hydrating
// a second editor from its older server copy.
export async function openDraftFromThreadWindow(draft: Message) {
  const draftId = normalizeMessageId(draft.message_id || draft.id)
  const heldId = normalizeMessageId(compose$.quickReplyDraftId.peek())
  const owner = compose$.quickReplyDraftThreadId.peek()
  if (draftId && heldId === draftId && owner) {
    ui$.calendarOpen.set(false)
    ui$.contactsOpen.set(false)
    compose$.conversationThread.set(owner)
    compose$.activeTab.set('')
    ui$.selectedThread.set(owner)
    ui$.mobilePane.set('conversation')
    ui$.replyFocus.set(ui$.replyFocus.peek() + 1)
    return true
  }
  return openDraftCompose(draft)
}

export async function openDraftConversationOrCompose(thread: Message) {
  if (!isDraftFolder(thread.folder_id, thread.account_id)) return false

  const release = reserveOpeningDraft(thread)
  try {
    if (await reopenAfterDraftSaves(thread)) return true
    const result = await invoke<{ messages: Message[] }>('mail.threadRead', {
      thread_id: thread.thread_id,
      limit: CONVERSATION_PAGE_SIZE,
    })
    const messages = result.messages ?? []
    const draft = newestMessage(messages.filter((message) => isDraftFolder(message.folder_id, message.account_id)))
    if (!draft) {
      showDraftNotFound()
      return true
    }

    // A saved reply already open in another window belongs to its editor,
    // even when this Drafts row would normally open the inline conversation.
    if (
      (await invoke<boolean>('threadWindow.focusDraft', { draft_id: draft.message_id || draft.id }).catch(
        () => false,
      )) === true
    )
      return true

    if (draftShouldOpenConversation(messages, draft)) {
      // This one opens a conversation, not an editor: hand the draft to the
      // quick reply by releasing before the messages land, or the hold that was
      // protecting it would suppress the very hydration that takes it over.
      release()
      compose$.activeTab.set('')
      ui$.selectedThread.set(thread.thread_id)
      ui$.mobilePane.set('conversation')
      if (ui$.selectedThread.peek() === thread.thread_id) {
        mail$.messages.set(messages)
        mail$.messagesCursor.set('')
        mail$.messagesCursorThread.set(thread.thread_id)
        mail$.messagesLoadingMore.set(false)
        mail$.threadLoading.set(false)
      }
      return true
    }

    if (accounts$.get().filter(isSendableAccount).length === 0) {
      showToast(t('compose.toast.addMailAccountBeforeComposing'))
      return true
    }
    const releaseResolved = reserveOpeningDraft(draft)
    try {
      await openDraftMessageInCompose(draft)
    } finally {
      releaseResolved()
    }
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('compose.toast.couldNotOpenDraft'), 'error')
  } finally {
    release()
  }
  return true
}
