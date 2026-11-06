import { observable } from '@legendapp/state'
import { t } from '../lib/i18n'
import type { ComposeDraft, ComposerAttachment, MessageTab } from '../types'
import { invoke } from '../lib/bridge'
import type { SignatureMark } from '../lib/signature'

// Compose/reader-tab + draft state. Reader tabs open using the account's
// conversation view preference; compose tabs hold a full-editor draft. The
// quick-reply composer (composer / composerAttachments) and per-thread quick
// drafts live here too. Persisted to localStorage (volatile editor buffers, not
// DB settings).

// Persisted full-editor drafts. Attachments are intentionally NOT persisted —
// their base64 payloads would quickly blow past localStorage's ~5MB budget.
// Only text fields survive restarts; the user reattaches files if needed.
export const COMPOSE_TABS_KEY = 'meron-compose-tabs'
// Which tab was on screen and the conversation behind the tabs, so a restart
// reopens where the user left off.
export const TAB_VIEW_KEY = 'meron-tab-view'

// Local placeholder used until the first save asks meron-core for the stable
// RFC Message-ID. It is never sent to IMAP/SMTP.
/**
 * A placeholder draft id, replaced by a server-allocated one on the first save.
 * Exported because a draft that moves to another account needs a new one: the
 * allocated id belongs to the account whose Drafts folder holds that copy.
 */
export const newDraftMessageId = () => `local-draft-${Date.now()}-${Math.random().toString(36).substring(2, 11)}`

export async function allocateMessageIdentity(accountId: string, draft: boolean): Promise<string> {
  const result = await invoke<{ message_id: string }>('mail.allocateIdentity', { account_id: accountId, draft })
  if (!result.message_id) throw new Error('Core did not allocate a message identity')
  return result.message_id
}

export type PersistedComposeTab = {
  id: string
  subject: string
  compose: Omit<ComposeDraft, 'attachments'> & { attachments: [] }
}
// Thread and reader tabs share the compose tabs' list so the strip keeps its
// order, but only as references: a mail body could use up the quota the drafts
// depend on. Reader tabs fetch theirs again at boot (restoreTabSession).
export type PersistedMailTab = Pick<
  MessageTab,
  'id' | 'messageId' | 'threadId' | 'accountId' | 'folderId' | 'subject' | 'from' | 'fromAddr' | 'feedIcon' | 'viewMode'
> & { kind: 'reader' | 'thread' }
type PersistedTabView = { activeTab?: unknown; conversationThread?: unknown }

export function hasExtraComposeHeaders(compose: Pick<ComposeDraft, 'cc' | 'bcc'>): boolean {
  return !!(compose.cc?.trim() || compose.bcc?.trim())
}
function loadPersistedTabs(): MessageTab[] {
  try {
    const raw = localStorage.getItem(COMPOSE_TABS_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as (PersistedComposeTab | PersistedMailTab)[]
    if (!Array.isArray(parsed)) return []
    return parsed.flatMap<MessageTab>((t) => {
      if (!t) return []
      if ('kind' in t) {
        if (!t.id || !t.threadId) return []
        return [{ ...t, body: '', bodyMissing: t.kind === 'reader' }]
      }
      if (!t.compose || typeof t.compose.to !== 'string') return []
      return [
        {
          id: t.id,
          kind: 'compose',
          messageId: '',
          threadId: '',
          subject: t.subject || t.compose.subject || 'New message',
          from: '',
          body: '',
          viewMode: 'plain',
          compose: {
            ...t.compose,
            fromEmail: t.compose.fromEmail ?? '',
            showCcBcc: t.compose.showCcBcc && hasExtraComposeHeaders(t.compose),
            // Backfill for tabs persisted before draftMessageId existed, so the
            // first autosave after restart still replaces rather than duplicates.
            draftMessageId: t.compose.draftMessageId || newDraftMessageId(),
            attachments: [],
          },
        },
      ]
    })
  } catch {
    return []
  }
}
function loadPersistedTabView(): PersistedTabView {
  try {
    const parsed = JSON.parse(localStorage.getItem(TAB_VIEW_KEY) ?? 'null') as PersistedTabView | null
    return parsed && typeof parsed === 'object' ? parsed : {}
  } catch {
    return {}
  }
}
const initialTabs = loadPersistedTabs()
const initialView = loadPersistedTabView()
const initialConversationThread =
  typeof initialView.conversationThread === 'string' ? initialView.conversationThread : ''
// The saved tab when it can still be shown, else the last one: with no tab
// active and nothing behind them, nothing would show the restored tabs, and the
// kanban pane would stay closed with them stranded out of sight.
const initialActiveTab = (() => {
  const saved = initialView.activeTab
  if (saved === '' && initialConversationThread) return ''
  if (typeof saved === 'string' && initialTabs.some((tab) => tab.id === saved)) return saved
  return initialTabs.at(-1)?.id ?? ''
})()

export const compose$ = observable({
  // Reader tabs for messages opened in "HTML mode"; activeTab "" = conversation view.
  // Re-hydrate the tabs persisted from the previous run.
  tabs: initialTabs,
  activeTab: initialActiveTab,
  // The thread shown by Current while a pinned thread retargets selectedThread
  // for actions and shortcuts. Current keeps its own mounted conversation.
  conversationThread: initialConversationThread,
  composer: '',
  composerAttachments: [] as ComposerAttachment[],
  // The quick reply's end-to-end encryption: null follows the default (on
  // when answering an encrypted message), true/false is the user's choice for
  // this thread (keyed by thread id so a thread switch drops it).
  quickReplyEncrypt: null as null | { threadId: string; on: boolean },
  // Server-side draft backing the active thread's quick reply, shared with the
  // full composer's saveDraft/discardDraft mechanism (mail.saveDraft/
  // mail.discardDraft) rather than a separate persistence path. Reset on
  // thread switch; re-derived from the thread's tail message when it's a
  // saved draft (see hydrateQuickReplyFromTailDraft below).
  quickReplyDraftId: '',
  quickReplyDraftSaved: false,
  // Exact owner of quickReplyDraftId. The selected thread can change before
  // its card or messages load, so neither collection is a reliable ownership
  // proxy when the quick-reply component mounts or remounts.
  quickReplyDraftThreadId: '',
  // Normalized ids of the server-side drafts a send has taken over, kept until
  // the post-send discard settles. The quick reply clears its own draft state
  // the moment it hands the text to the send, so without this a background
  // thread refresh landing in that window would render the still-cached draft
  // right above the message it was just sent as.
  sendingDraftIds: [] as string[],
  // Send-as address explicitly chosen for the active thread's quick reply, set
  // by the From indicator's picker. Empty means "auto" — fall back to the alias
  // the original was delivered to (detectAliasFrom). Reset on thread switch, so
  // an override never leaks into the next conversation.
  quickReplyFrom: '',
  // People Reply all would reach whom the user added to the quick reply's Cc
  // from its recipient line (or a draft saved that way), as bare lowercased
  // addresses. Reset like quickReplyFrom on a thread switch, and after a send
  // or an escalation to the full editor.
  quickReplyAdded: [] as string[],
  // The signature this app seeded into the quick reply, or null when there is
  // none to account for — the account sends none, or the body was hydrated from
  // a saved draft that already carries its own.
  //
  // Unlike the full composer's tracking (see lib/signature's SignatureTracking)
  // this needs no third "inserted nothing, but managed" state: a quick reply
  // cannot change sending account, since its From picker only offers aliases of
  // the one account and those all share a signature. With nothing to swap, the
  // only question ever asked of this is which part of the box is the user's.
  quickReplySignature: null as SignatureMark | null,
})
