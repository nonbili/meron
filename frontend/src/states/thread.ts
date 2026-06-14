import { observable } from '@legendapp/state'

export type ConversationMode = 'plain' | 'html'

// UI state scoped to the active conversation view (MessagePane and its children).
// Kept separate from app$ so per-thread/per-bubble interactions don't add noise
// to the global store.
export const thread$ = observable({
  search: '',
  searchOpen: false,
  activeSearchIndex: 0,
  // Message id of the currently-focused search match; written by MessagePane
  // when search/index change, read by MessageBubble for highlight styling.
  activeSearchId: '',
  // Message id to scroll to when its thread next renders — set by the starred
  // list before selecting the thread, consumed once by useConversationScroll.
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

export function revealRemote(messageId: string) {
  thread$.revealedRemote[messageId].set(true)
}

export function resetThreadView() {
  thread$.search.set('')
  thread$.searchOpen.set(false)
  thread$.activeSearchIndex.set(0)
  // pendingScrollMessageId deliberately survives: it's set just before the
  // thread switch that triggers this reset.
  thread$.flashMessageId.set('')
  thread$.revealedRemote.set({})
  thread$.galleryIndex.set(null)
}
