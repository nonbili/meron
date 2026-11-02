import { afterEach, beforeEach, describe, expect, it, spyOn } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { ConversationTabs } from './ConversationTabs'
import { openThreadTab } from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { kanban$ } from '../../states/kanban'
import { ui$ } from '../../states/ui'
import type { Message } from '../../types'

const message = (over: Partial<Message> = {}): Message =>
  ({
    id: 'm1',
    account_id: 'acct',
    folder_id: 'INBOX',
    thread_id: 't-tab',
    from_name: 'A',
    from_addr: 'a@example.com',
    to: '',
    subject: 'Task mail',
    preview: '',
    body: '',
    date: 1,
    unread: false,
    starred: false,
    has_attachments: false,
    ...over,
  }) as Message

describe('ConversationTabs', () => {
  beforeEach(() => {
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.conversationThread.set('')
    ui$.selectedThread.set('')
    kanban$.activeBoardId.set('')
    kanban$.paneThreadId.set('')
  })
  afterEach(cleanup)

  it('offers the Current tab while a conversation sits behind the tabs', () => {
    ui$.selectedThread.set('t-current')
    openThreadTab(message())
    const view = render(<ConversationTabs />)
    expect(view.queryByTitle('Current conversation')).not.toBeNull()
  })

  it('activates Current when its padding is clicked', () => {
    ui$.selectedThread.set('t-current')
    openThreadTab(message())
    const view = render(<ConversationTabs />)

    fireEvent.click(view.getByTitle('Current conversation'))

    expect(compose$.activeTab.get()).toBe('')
    expect(ui$.selectedThread.get()).toBe('t-current')
  })

  it('closes Current without activating it when its close button is clicked', () => {
    ui$.selectedThread.set('t-current')
    openThreadTab(message())
    const tabId = compose$.activeTab.get()
    const view = render(<ConversationTabs />)

    fireEvent.click(view.getByTitle('Current conversation').querySelector('[aria-label="Close tab"]')!)

    expect(compose$.activeTab.get()).toBe(tabId)
    expect(compose$.conversationThread.get()).toBe('')
  })

  it('hides the Current tab when no conversation is open behind the tabs', () => {
    // A task's mail opened with an empty pane: there is nothing to go back to,
    // and in kanban view the Current tab would close the pane outright.
    openThreadTab(message())
    const view = render(<ConversationTabs />)
    expect(view.queryByTitle('Current conversation')).toBeNull()
    expect(view.queryByTitle('Task mail')).not.toBeNull()
  })

  it('follows the kanban pane rather than the remembered thread on a board', () => {
    kanban$.activeBoardId.set('board-1')
    ui$.selectedThread.set('t-current')
    openThreadTab(message())
    const view = render(<ConversationTabs />)
    expect(view.queryByTitle('Current conversation')).toBeNull()

    act(() => kanban$.paneThreadId.set('t-card'))
    expect(view.queryByTitle('Current conversation')).not.toBeNull()
  })

  it('closes the Current conversation along with the tabs from its menu', () => {
    ui$.selectedThread.set('t-current')
    openThreadTab(message())
    openThreadTab(message({ thread_id: 't-other', subject: 'Other mail' }))
    const view = render(<ConversationTabs />)

    fireEvent.contextMenu(view.getByTitle('Current conversation'))
    // Current sits first: nothing lies to its left.
    expect((view.getByText('Close tabs to the left').closest('button') as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(view.getByText('Close all tabs'))

    expect(compose$.tabs.get()).toHaveLength(0)
    expect(compose$.conversationThread.get()).toBe('')
    expect(ui$.selectedThread.get()).toBe('')
  })

  it("closes only the tabs from Current's close-others entry", () => {
    ui$.selectedThread.set('t-current')
    openThreadTab(message())
    const view = render(<ConversationTabs />)

    fireEvent.contextMenu(view.getByTitle('Current conversation'))
    fireEvent.click(view.getByText('Close other tabs'))

    expect(compose$.tabs.get()).toHaveLength(0)
    expect(compose$.activeTab.get()).toBe('')
    expect(ui$.selectedThread.get()).toBe('t-current')
  })

  it('reorders tabs by dragging, leaving the active tab and Current alone', async () => {
    ui$.selectedThread.set('t-current')
    openThreadTab(message({ thread_id: 't-a', subject: 'First' }))
    openThreadTab(message({ thread_id: 't-b', subject: 'Second' }))
    const rect = spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
      const tab = this.closest('[data-tab-id]')
      const left = tab ? [...tab.parentElement!.children].indexOf(tab) * 100 : 0
      return new DOMRect(left, 0, 100, 36)
    })
    const view = render(<ConversationTabs />)
    const first = view.getByTitle('First')

    fireEvent.mouseDown(first, { clientX: 150, clientY: 10, button: 0 })
    fireEvent.mouseMove(document, { clientX: 170, clientY: 10 })
    fireEvent.mouseMove(document, { clientX: 260, clientY: 10 })
    await waitFor(() => expect(first.className).toContain('z-20'))
    fireEvent.mouseUp(document)
    rect.mockRestore()
    // dnd-kit retains its document click listener for 50ms after dragging.
    await new Promise((resolve) => setTimeout(resolve, 60))

    expect(compose$.tabs.get().map((tab) => tab.threadId)).toEqual(['t-b', 't-a'])
    expect(compose$.activeTab.get()).toBe('thread-t-b')
    const strip = [...document.querySelectorAll('[data-tab-id]')].map((tab) => tab.getAttribute('data-tab-id'))
    expect(strip).toEqual(['', 'thread-t-b', 'thread-t-a'])
  })

  it('does not start a drag from the close button', async () => {
    openThreadTab(message({ thread_id: 't-a', subject: 'First' }))
    openThreadTab(message({ thread_id: 't-b', subject: 'Second' }))
    const view = render(<ConversationTabs />)
    const first = view.getByTitle('First')

    fireEvent.mouseDown(first.querySelector('button')!, { clientX: 10, clientY: 10, button: 0 })
    fireEvent.mouseMove(document, { clientX: 200, clientY: 10 })
    fireEvent.mouseUp(document)

    expect(first.className).not.toContain('z-20')
    expect(compose$.tabs.get().map((tab) => tab.threadId)).toEqual(['t-a', 't-b'])
  })
})
