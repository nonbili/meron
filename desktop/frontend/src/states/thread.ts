import { observable } from '@legendapp/state'

export type ConversationMode = 'plain' | 'html'

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
  // Message id to scroll to when its thread next renders — set by direct-jump
  // actions such as starred items and shared media, consumed once by
  // useConversationScroll.
  pendingScrollMessageId: '',
  // Message id briefly ring-highlighted after such a jump.
  flashMessageId: '',
  // Message ids whose remote images the user revealed for this session.
  revealedRemote: {} as Record<string, boolean>,
  // Index into galleryItems for the lightbox; null when closed.
  galleryIndex: null as number | null,
  mediaOpen: false,
  // Per-account override of the conversation render mode for this session.
  conversationModeOverrides: {} as Record<string, ConversationMode>,
})

export function openThreadSearch() {
  thread$.searchOpen.set(true)
  thread$.searchFocus.set(thread$.searchFocus.peek() + 1)
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
  thread$.revealedRemote.set({})
  thread$.galleryIndex.set(null)
}
