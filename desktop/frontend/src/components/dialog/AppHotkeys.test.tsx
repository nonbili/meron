import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { cleanup, fireEvent, render } from '@testing-library/react'
import { isMac } from '../../lib/shortcuts'
import { calendar$ } from '../../states/calendar'
import { mail$ } from '../../states/mailState'
import { ui$ } from '../../states/ui'
import { kanban$, kanbanBoardColumnKey, kanbanColumnKey } from '../../states/kanban'
import { settings$ } from '../../states/settings'
import { compose$ } from '../../states/composeState'
import { AppHotkeys } from './AppHotkeys'

const original = (window as any).go
let calls: string[] = []

beforeEach(() => {
  calls = []
  ;(window as any).go = { main: { App: { Invoke: async (command: string) => void calls.push(command) } } }
  ui$.selectedAccount.set('acct')
  ui$.selectedFolder.set('inbox')
  ui$.selectedThread.set('acct#INBOX#7')
  mail$.threads.set([{ id: 'acct#INBOX#7', account_id: 'acct', folder_id: 'inbox' } as any])
})

afterEach(() => {
  cleanup()
  ui$.calendarOpen.set(false)
  ui$.calendarPanelOpen.set(false)
  calendar$.editor.set(null)
  ui$.selectedThread.set('')
  mail$.threads.set([])
  ;(window as any).go = original
})

describe('AppHotkeys', () => {
  it('keeps mail shortcuts off the mail the calendar hides', async () => {
    ui$.calendarOpen.set(true)
    render(<AppHotkeys />)
    fireEvent.keyDown(document.body, { key: 'e' })
    fireEvent.keyDown(document.body, { key: '#', shiftKey: true })
    fireEvent.keyDown(document.body, { key: 'Delete' })
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(calls.filter((command) => command.startsWith('mail.'))).toEqual([])
    expect(ui$.selectedThread.peek()).toBe('acct#INBOX#7')
  })

  it('keeps mail shortcuts off the mail under the event editor from the panel', async () => {
    ui$.calendarPanelOpen.set(true)
    calendar$.editor.set({ mode: 'new', day: Date.now() })
    render(<AppHotkeys />)
    fireEvent.keyDown(document.body, { key: 'e' })
    fireEvent.keyDown(document.body, { key: 'Delete' })
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(calls.filter((command) => command.startsWith('mail.'))).toEqual([])
  })

  it('keeps rail shortcuts from leaving an open event editor', async () => {
    ui$.calendarOpen.set(true)
    calendar$.editor.set({ mode: 'new', day: Date.now() })
    render(<AppHotkeys />)
    fireEvent.keyDown(document.body, { key: '1', ctrlKey: !isMac, metaKey: isMac })
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(ui$.calendarOpen.peek()).toBe(true)
    expect(calendar$.editor.peek()).not.toBeNull()
  })

  it('still archives the selected thread with the calendar closed', async () => {
    render(<AppHotkeys />)
    fireEvent.keyDown(document.body, { key: 'e' })
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(calls).toContain('mail.archive')
  })

  it('steps threads with Alt+arrows from the message, but not from a text field', () => {
    mail$.threads.set([
      { id: 'acct#INBOX#7', thread_id: 'acct#INBOX#7', account_id: 'acct', folder_id: 'inbox' } as any,
      { id: 'acct#INBOX#8', thread_id: 'acct#INBOX#8', account_id: 'acct', folder_id: 'inbox' } as any,
    ])
    const view = render(
      <>
        <AppHotkeys />
        <textarea aria-label="reply" />
        <div data-message-scroller tabIndex={-1}>
          <p>body</p>
        </div>
      </>,
    )
    fireEvent.keyDown(view.getByLabelText('reply'), { key: 'ArrowDown', altKey: true })
    expect(ui$.selectedThread.peek()).toBe('acct#INBOX#7')

    fireEvent.keyDown(view.getByText('body'), { key: 'ArrowDown', altKey: true })
    expect(ui$.selectedThread.peek()).toBe('acct#INBOX#8')
  })

  it('hands focus back to the selected thread row on Esc in the message', () => {
    const view = render(
      <>
        <AppHotkeys />
        <section data-thread-list>
          <button data-thread-row>other</button>
          <button data-thread-row data-active>
            selected
          </button>
        </section>
        <div data-message-scroller tabIndex={-1}>
          <p>body</p>
        </div>
      </>,
    )
    // happy-dom lays nothing out; any rect counts the row as on screen.
    view.getByText('selected').getClientRects = () => [{}] as unknown as DOMRectList
    const notCanceled = fireEvent.keyDown(view.getByText('body'), { key: 'Escape' })
    expect(notCanceled).toBe(false)
    expect(document.activeElement).toBe(view.getByText('selected'))
  })

  it('keeps the arrow keys on the message after its focused card unmounts', () => {
    mail$.threads.set([
      { id: 'acct#INBOX#7', thread_id: 'acct#INBOX#7', account_id: 'acct', folder_id: 'inbox' } as any,
      { id: 'acct#INBOX#8', thread_id: 'acct#INBOX#8', account_id: 'acct', folder_id: 'inbox' } as any,
    ])
    const view = render(
      <>
        <AppHotkeys />
        <div data-message-scroller data-testid="scroller" tabIndex={-1}>
          <button>card</button>
        </div>
      </>,
    )
    const scroller = view.getByTestId('scroller')
    // happy-dom lays nothing out; any rect counts the scroller as on screen.
    scroller.getClientRects = () => [{}] as unknown as DOMRectList
    const scrolls: unknown[] = []
    scroller.scrollBy = ((options: unknown) => void scrolls.push(options)) as typeof scroller.scrollBy

    // Expanding a message swaps its card, dropping focus to <body>.
    const card = view.getByText('card')
    card.focus()
    card.remove()
    expect(document.activeElement).toBe(document.body)
    expect(fireEvent.keyDown(document.body, { key: 'ArrowDown' })).toBe(false)
    expect(scrolls).toHaveLength(1)
    expect(ui$.selectedThread.peek()).toBe('acct#INBOX#7')

    // A click outside the message hands the arrows back to the list.
    fireEvent.pointerDown(document.body)
    fireEvent.keyDown(document.body, { key: 'ArrowDown' })
    expect(ui$.selectedThread.peek()).toBe('acct#INBOX#8')
  })

  it('leaves the arrow keys to the message while focus is in it', () => {
    mail$.threads.set([
      { id: 'acct#INBOX#7', thread_id: 'acct#INBOX#7', account_id: 'acct', folder_id: 'inbox' } as any,
      { id: 'acct#INBOX#8', thread_id: 'acct#INBOX#8', account_id: 'acct', folder_id: 'inbox' } as any,
    ])
    const view = render(
      <>
        <AppHotkeys />
        <div data-message-scroller tabIndex={-1}>
          <p>body</p>
        </div>
      </>,
    )
    const notCanceled = fireEvent.keyDown(view.getByText('body'), { key: 'ArrowDown' })
    expect(notCanceled).toBe(true)
    expect(ui$.selectedThread.peek()).toBe('acct#INBOX#7')

    fireEvent.keyDown(document.body, { key: 'ArrowDown' })
    expect(ui$.selectedThread.peek()).toBe('acct#INBOX#8')
  })

  it("hands focus to the closed conversation's card when Esc closes the kanban pane", () => {
    kanban$.activeBoardId.set('board-1')
    kanban$.paneThreadId.set('acct#INBOX#7')
    kanban$.paneColumnKey.set('board-1:col-b')
    try {
      const view = render(
        <>
          <AppHotkeys />
          <section data-thread-list data-kanban-column="board-1:col-a">
            <button data-thread-row data-active>
              same thread, other column
            </button>
          </section>
          <section data-thread-list data-kanban-column="board-1:col-b">
            <button data-thread-row>other</button>
            <button data-thread-row data-active>
              opened card
            </button>
          </section>
          <div data-message-scroller tabIndex={-1}>
            <p>body</p>
          </div>
        </>,
      )
      fireEvent.keyDown(view.getByText('body'), { key: 'Escape' })
      expect(kanban$.paneThreadId.peek()).toBe('')
      expect(document.activeElement).toBe(view.getByText('opened card'))
      expect(kanban$.keyboardCard.peek()).toEqual({ threadId: 'acct#INBOX#7', columnKey: 'board-1:col-b' })
    } finally {
      kanban$.activeBoardId.set('')
      kanban$.paneThreadId.set('')
      kanban$.paneColumnKey.set('')
      kanban$.keyboardCard.set(null)
    }
  })

  it("hands focus to the closed conversation's card when Esc comes from a message frame", () => {
    kanban$.activeBoardId.set('board-1')
    kanban$.paneThreadId.set('acct#INBOX#7')
    kanban$.paneColumnKey.set('board-1:col-b')
    try {
      const view = render(
        <>
          <AppHotkeys />
          <section data-thread-list data-kanban-column="board-1:col-b">
            <button data-thread-row data-active>
              opened card
            </button>
          </section>
        </>,
      )
      window.dispatchEvent(new CustomEvent('meron.frameKeyDown', { detail: { key: 'Escape' }, cancelable: true }))
      expect(kanban$.paneThreadId.peek()).toBe('')
      expect(document.activeElement).toBe(view.getByText('opened card'))
    } finally {
      kanban$.activeBoardId.set('')
      kanban$.paneThreadId.set('')
      kanban$.paneColumnKey.set('')
    }
  })

  it('steps from the focused card while no conversation is open', () => {
    const inbox = { accountId: 'acct', folderId: 'inbox' }
    const card = (n: number) => ({ id: `m${n}`, thread_id: `t${n}`, account_id: 'acct', folder_id: 'inbox' }) as any
    const previousBoards = settings$.kanbanBoards.peek()
    settings$.kanbanBoards.set([{ id: 'b1', name: 'Board', columns: [inbox] }] as any)
    kanban$.activeBoardId.set('b1')
    kanban$.threads.set({ [kanbanColumnKey(inbox)]: [card(1), card(2), card(3), card(4)] })
    kanban$.paneThreadId.set('')
    kanban$.paneColumnKey.set('')
    compose$.tabs.set([])
    compose$.activeTab.set('')
    ui$.selectedThread.set('')
    try {
      const view = render(
        <>
          <AppHotkeys />
          <section data-thread-list data-kanban-column={kanbanBoardColumnKey('b1', inbox)}>
            <div data-kanban-thread="t2">
              <button data-thread-row>card 2</button>
            </div>
          </section>
        </>,
      )
      view.getByText('card 2').focus()
      fireEvent.keyDown(view.getByText('card 2'), { key: 'ArrowDown' })
      expect(kanban$.paneThreadId.peek()).toBe('t3')

      kanban$.paneThreadId.set('')
      ui$.selectedThread.set('')
      view.getByText('card 2').focus()
      fireEvent.keyDown(view.getByText('card 2'), { key: 'j' })
      expect(kanban$.paneThreadId.peek()).toBe('t3')
    } finally {
      settings$.kanbanBoards.set(previousBoards)
      kanban$.activeBoardId.set('')
      kanban$.threads.set({})
      kanban$.paneThreadId.set('')
      kanban$.paneColumnKey.set('')
    }
  })
})
