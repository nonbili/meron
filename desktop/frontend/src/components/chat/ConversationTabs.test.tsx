import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, render } from '@testing-library/react'
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
})
