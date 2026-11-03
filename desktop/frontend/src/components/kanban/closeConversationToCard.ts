import { closeCurrentConversation, kanban$ } from '../../states/kanban'

/** Close the card conversation, as Esc does, and hand keyboard focus to the
 * card it was opened from, which keeps looking selected, so the closed thread
 * is still the one the keyboard is on. The card is found before closing: it stops being the active one then.
 * A thread can sit in two columns of a board, so the pane's own column decides. */
export function closeConversationToCard() {
  const columnKey = kanban$.paneColumnKey.peek()
  const column = [...document.querySelectorAll<HTMLElement>('[data-kanban-column]')].find(
    (element) => element.dataset.kanbanColumn === columnKey,
  )
  const card = column?.querySelector<HTMLElement>('[data-thread-row][data-active]')
  const threadId = kanban$.paneThreadId.peek()
  closeCurrentConversation()
  if (!card?.isConnected) return
  // The card stays selected-looking while it has the keyboard (KanbanThreadCard).
  kanban$.keyboardCard.set({ threadId, columnKey })
  card.focus()
}
