import { compose$ } from '../../states/composeState'
import { closeCurrentConversation, kanban$ } from '../../states/kanban'

/** Close the card conversation, as Esc does, and hand keyboard focus to the
 * card it was opened from, which keeps looking selected (kanban$.keyboardCard),
 * so the closed thread is still the one the keyboard is on. The card is found
 * before closing, while it is still the active one. A thread can sit in two
 * columns of a board, so the pane's own column decides. With a tab on screen
 * after closing, focus stays where it is. */
export function closeConversationToCard() {
  const columnKey = kanban$.paneColumnKey.peek()
  const column = [...document.querySelectorAll<HTMLElement>('[data-kanban-column]')].find(
    (element) => element.dataset.kanbanColumn === columnKey,
  )
  const card = column?.querySelector<HTMLElement>('[data-thread-row][data-active]')
  const threadId = kanban$.paneThreadId.peek()
  closeCurrentConversation()
  // A tab on screen (one already in front, or one that closing brought forward)
  // owns the pane and the keyboard; the card isn't shown as selected behind it.
  if (compose$.activeTab.peek()) return
  if (!card?.isConnected) return
  kanban$.keyboardCard.set({ threadId, columnKey })
  card.focus()
}
