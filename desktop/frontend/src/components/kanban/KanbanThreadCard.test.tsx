import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, render } from '@testing-library/react'
import { DndContext } from '@dnd-kit/core'
import { compose$ } from '../../states/composeState'
import { kanban$, kanbanBoardColumnKey, type KanbanColumn } from '../../states/kanban'
import type { Message } from '../../types'
import type { ThreadContextMenuController } from '../threads/ThreadContextMenu'
import { KanbanThreadCard } from './KanbanThreadCard'

const thread: Message = {
  id: 'root',
  account_id: 'acc-1',
  folder_id: 'INBOX',
  thread_id: 't-1',
  from_name: 'Them',
  from_addr: 'them@example.com',
  to: 'me@example.com',
  message_id: 'root@example.com',
  subject: 'Subject',
  preview: 'Original',
  body: 'Original',
  date: 1,
  unread: false,
  starred: false,
  has_attachments: false,
}

const inbox = { accountId: 'acc-1', folderId: 'INBOX' } as KanbanColumn
const threadMenu: ThreadContextMenuController = { menu: null, close: () => {}, isOpen: () => false, open: () => {} }

function renderTwoCards() {
  const second = { ...thread, id: 'root-2', thread_id: 't-2', subject: 'Second' }
  const view = render(
    <DndContext>
      <KanbanThreadCard boardId="board-1" thread={thread} column={inbox} threadMenu={threadMenu} ownerKey="col-1" />
      <KanbanThreadCard boardId="board-1" thread={second} column={inbox} threadMenu={threadMenu} ownerKey="col-1" />
    </DndContext>,
  )
  const row = (text: string) => view.getByText(text).closest<HTMLElement>('[data-thread-row]')!
  return { first: row('Subject'), second: row('Second') }
}

function renderCard(column: KanbanColumn = inbox) {
  const view = render(
    <DndContext>
      <KanbanThreadCard boardId="board-1" thread={thread} column={column} threadMenu={threadMenu} ownerKey="col-1" />
      <button>elsewhere</button>
    </DndContext>,
  )
  return { view, row: view.getByText('Subject').closest<HTMLElement>('[data-thread-row]')! }
}

describe('KanbanThreadCard after Esc', () => {
  beforeEach(() => {
    kanban$.paneThreadId.set('')
    kanban$.keyboardCard.set(null)
    compose$.tabs.set([])
    compose$.activeTab.set('')
  })

  afterEach(() => {
    cleanup()
    kanban$.keyboardCard.set(null)
  })

  it('shows the card Esc left the keyboard on as selected, though its conversation is closed', () => {
    kanban$.keyboardCard.set({ threadId: 't-1', columnKey: kanbanBoardColumnKey('board-1', inbox) })
    const { row } = renderCard()
    expect(row.hasAttribute('data-active')).toBe(true)
  })

  it('leaves the same thread unselected in another column', () => {
    kanban$.keyboardCard.set({ threadId: 't-1', columnKey: kanbanBoardColumnKey('board-1', inbox) })
    const { row } = renderCard({ accountId: 'unified', folderId: 'inbox' } as KanbanColumn)
    expect(row.hasAttribute('data-active')).toBe(false)
  })

  it('stops showing it once focus moves elsewhere in the window', () => {
    kanban$.keyboardCard.set({ threadId: 't-1', columnKey: kanbanBoardColumnKey('board-1', inbox) })
    const { view, row } = renderCard()
    row.focus()
    view.getByText('elsewhere').focus()
    expect(kanban$.keyboardCard.get()).toBeNull()
  })

  it('takes keyboard focus when the arrow keys select it, so Return opens it', () => {
    const { first, second } = renderTwoCards()
    act(() => kanban$.keyboardCard.set({ threadId: 't-1', columnKey: kanbanBoardColumnKey('board-1', inbox) }))
    expect(document.activeElement).toBe(first)

    act(() => kanban$.keyboardCard.set({ threadId: 't-2', columnKey: kanbanBoardColumnKey('board-1', inbox) }))
    expect(document.activeElement).toBe(second)
    // The card it moved away from must not drop the new selection as it blurs.
    expect(kanban$.keyboardCard.get()?.threadId).toBe('t-2')
    expect(second.hasAttribute('data-active')).toBe(true)
    expect(first.hasAttribute('data-active')).toBe(false)
  })
})
