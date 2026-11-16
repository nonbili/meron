import { afterEach, expect, it, mock } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import * as dndCore from '@dnd-kit/core'
import { accounts$ } from '../../states/accounts'
import { kanban$, kanbanColumnKey, type KanbanColumn } from '../../states/kanban'
import { settings$ } from '../../states/settings'
import type { Account, Message } from '../../types'

// Every card is a useDraggable consumer, so counting its calls counts card renders.
// Copied before the module is replaced, or the wrapper would call itself.
const realDnd = { ...dndCore }
let cardRenders = 0
mock.module('@dnd-kit/core', () => ({
  ...realDnd,
  useDraggable: (args: Parameters<typeof dndCore.useDraggable>[0]) => {
    cardRenders++
    return realDnd.useDraggable(args)
  },
}))

const { KanbanView } = await import('./KanbanView')

const column: KanbanColumn = { accountId: 'acc-1', folderId: 'INBOX' }
const threads: Message[] = Array.from({ length: 20 }, (_, i) => ({
  id: `m-${i}`,
  account_id: 'acc-1',
  folder_id: 'INBOX',
  thread_id: `t-${i}`,
  from_name: 'Alice',
  from_addr: 'alice@example.com',
  to: 'bob@example.com',
  message_id: `m-${i}@example.com`,
  subject: `Subject ${i}`,
  preview: '',
  body: '',
  date: i,
  unread: false,
  starred: false,
  has_attachments: false,
}))

const previousGo = (window as any).go
afterEach(() => {
  cleanup()
  ;(window as any).go = previousGo
})

it('opening a card menu re-renders only that card, not the whole board', async () => {
  ;(window as any).go = { main: { App: { Invoke: async () => ({ threads: [], folders: [] }) } } }
  accounts$.set([{ id: 'acc-1', email: 'bob@example.com', provider: 'imap' } as Account])
  settings$.kanbanBoards.set([{ id: 'board-1', name: 'Board', columns: [column] }])
  const view = render(<KanbanView boardId="board-1" />)
  // The column's first load settles empty against the stub; then fill it.
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20))
  })
  act(() => {
    kanban$.threads[kanbanColumnKey(column)].set(threads)
    kanban$.loading[kanbanColumnKey(column)].set(false)
  })
  const rows = view.container.querySelectorAll('[data-thread-row]')
  expect(rows.length).toBe(threads.length)

  cardRenders = 0
  act(() => {
    fireEvent.contextMenu(rows[3])
  })
  // The menu state lives on the board, which re-renders; dnd-kit sensors built
  // from inline options would then re-render every card through its context.
  expect(cardRenders).toBe(1)
})
