import { observable } from '@legendapp/state'
import type { Account, Message } from '../types'

export type ConversationMode = 'plain' | 'html'

export type MessageContextMenuState = {
  x: number
  y: number
  message: Message
  linkUrl?: string
  // A right-clicked image attachment; the menu then offers save/copy for it.
  image?: { key: string | null; url?: string; filename: string; mime?: string }
  hideOpenInNewTab?: boolean
}

// UI state scoped to the active conversation view (MessagePane and its children).
// Kept separate from app$ so per-thread/per-bubble interactions don't add noise
// to the global store.
export const thread$ = observable({
  search: '',
  searchOpen: false,
  // Bumped to pull focus into the thread search input, e.g. when ⌘/Ctrl+F is
  // pressed while the bar is already open (opening it alone focuses it).
  searchFocus: 0,
  // Index of the focused match among every occurrence in the thread — not among
  // matching messages: one message can hold several.
  activeSearchIndex: 0,
  // Message id of the currently-focused search match; written by MessagePane
  // when search/index change, read by MessageBubble for highlight styling.
  activeSearchId: '',
  // Which occurrence within that message is focused (-1 when the message
  // matches only in its subject or sender, so no <mark> is the active one).
  activeSearchOffset: -1,
  // Total occurrences of the query in the thread; published by useThreadSearch
  // for the two search bars' counter and prev/next buttons.
  searchMatchCount: 0,
  // Message id to scroll to when its thread next renders — set by direct-jump
  // actions such as starred items and shared media, consumed once by
  // useConversationScroll.
  pendingScrollMessageId: '',
  // Id of the bubble a send just put in the open conversation. The reader
  // wrote it, so the view follows it wherever they had scrolled to — consumed
  // once by useConversationScroll.
  pendingOwnSendId: '',
  // Message id briefly ring-highlighted after such a jump.
  flashMessageId: '',
  // Remote images revealed in the active view. New views also inherit the
  // session's reveals; existing hidden views keep their own live documents.
  revealedRemote: {} as Record<string, boolean>,
  // Index into galleryItems for the lightbox; null when closed.
  galleryIndex: null as number | null,
  mediaOpen: false,
  // URL of the link under the pointer in a message body, shown in the pane's
  // status-bar style preview.
  hoveredLink: null as string | null,
  // The open right-click / "more" menu for a message; null when closed.
  messageContextMenu: null as MessageContextMenuState | null,
  // The thread's actions menu opened at the pointer, by a right-click on the
  // blank part of the conversation; null when closed. The header owns the menu.
  conversationMenu: null as { x: number; y: number } | null,
  // Traditional layout: the reader's last expand-all or collapse-all in the open
  // thread. Every message without a toggle of its own follows it, earlier ones
  // loaded afterwards included. `seq` makes every request a change, so asking
  // again for what is already in force still resets the messages toggled since.
  expandAll: null as { expanded: boolean; seq: number } | null,
  // What the menu's expand-all / collapse-all item would do now. Published by
  // ConversationMessageList; null where there is nothing to offer (the chat
  // layout, a thread of one message that is already open).
  expandAllAction: null as 'expand' | 'collapse' | null,
  // Per-account override of the conversation render mode for this session.
  conversationModeOverrides: {} as Record<string, ConversationMode>,
})

// The active view publishes its reveal map to thread$, while navigation can
// still seed a newly mounted view from everything revealed in this session.
const sessionReveals: Record<string, boolean> = {}
thread$.revealedRemote.onChange(({ value }) => {
  for (const [id, revealed] of Object.entries(value)) if (revealed) sessionReveals[id] = true
})
export function sessionRemoteReveals() {
  return { ...sessionReveals }
}
export function forgetRemoteReveals() {
  for (const id of Object.keys(sessionReveals)) delete sessionReveals[id]
  thread$.revealedRemote.set({})
}

export function openThreadSearch() {
  thread$.searchOpen.set(true)
  thread$.searchFocus.set(thread$.searchFocus.peek() + 1)
}

/** Steps the in-thread search to the previous/next occurrence, wrapping round. */
export function goToSearchMatch(direction: -1 | 1) {
  const matchCount = thread$.searchMatchCount.peek()
  if (matchCount === 0) return
  const next = thread$.activeSearchIndex.peek() + direction
  thread$.activeSearchIndex.set(next < 0 ? matchCount - 1 : next >= matchCount ? 0 : next)
}

export function setAllMessagesExpanded(expanded: boolean) {
  thread$.expandAll.set({ expanded, seq: (thread$.expandAll.peek()?.seq ?? 0) + 1 })
}

export function revealRemote(messageId: string) {
  thread$.revealedRemote[messageId].set(true)
}

export function resetThreadView() {
  thread$.search.set('')
  thread$.searchOpen.set(false)
  thread$.activeSearchIndex.set(0)
  thread$.activeSearchId.set('')
  thread$.activeSearchOffset.set(-1)
  // pendingScrollMessageId deliberately survives: it's set just before the
  // thread switch that triggers this reset.
  thread$.flashMessageId.set('')
  thread$.galleryIndex.set(null)
  thread$.hoveredLink.set(null)
  thread$.messageContextMenu.set(null)
  thread$.conversationMenu.set(null)
  thread$.expandAll.set(null)
}

/** The render mode for an account: the session override when the user flipped
 *  it in the header, otherwise the account setting. */
export function resolveConversationMode(
  accounts: Account[],
  modeOverrides: Record<string, ConversationMode>,
  accountId: string | undefined,
): ConversationMode {
  const account = accountId ? accounts.find((acc) => acc.id === accountId) : null
  if (!account) return 'plain'
  return modeOverrides[account.id] ?? ((account.conversation_html ?? true) ? 'html' : 'plain')
}
