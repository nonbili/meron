import { afterEach, describe, expect, it } from 'bun:test'
import { cleanup, render } from '@testing-library/react'
import { createRef } from 'react'
// Initialize this side of the mail/compose cycle before ConversationMessageList
// pulls both modules in through its message actions and mail paging imports.
import '../../states/compose'
import { settings$ } from '../../states/settings'
import type { Message } from '../../types'
import { ConversationMessageList } from './ConversationMessageList'

function message(id: string, body: string): Message {
  return {
    id,
    account_id: 'account-1',
    folder_id: 'inbox',
    thread_id: 'thread-1',
    from_name: 'Sender',
    from_addr: 'sender@example.com',
    to: 'me@example.com',
    subject: 'Subject',
    preview: body,
    body,
    date: 0,
    unread: false,
    starred: false,
    has_attachments: false,
  }
}

afterEach(() => {
  cleanup()
  settings$.conversationLayout.set('chat')
})

describe('ConversationMessageList direct jumps', () => {
  it('keeps an older jumped-to message expanded after its highlight ends', () => {
    settings$.conversationLayout.set('traditional')
    const messages = [message('older', 'Older body'), message('newer', 'Newer body')]
    const scrollRef = createRef<HTMLDivElement>()
    const messagesWrapperRef = createRef<HTMLDivElement>()
    const bottomAnchorRef = createRef<HTMLDivElement>()
    const commonProps = {
      messages,
      showThreadLoading: false,
      showThreadError: false,
      onRetryThreadLoad: () => undefined,
      messagesCursor: '',
      messagesLoadingMore: false,
      activeThreadId: 'thread-1',
      searchMatches: [],
      activeSearchId: '',
      galleryOffsets: new Map<string, number>(),
      scrollRef,
      messagesWrapperRef,
      bottomAnchorRef,
      wallpaperClassName: '',
      onScroll: () => undefined,
      onSetScrollTop: () => undefined,
      onScrollMessageToTop: () => undefined,
      onOpenContextMenu: () => undefined,
    }

    const view = render(<ConversationMessageList {...commonProps} jumpMessageId="" />)
    const older = view.container.querySelector<HTMLElement>('[data-message-id="older"]')!
    expect(older.querySelector('[title="Expand message"]')).not.toBeNull()

    view.rerender(<ConversationMessageList {...commonProps} jumpMessageId="older" />)
    expect(older.querySelector('[title="Collapse message"]')).not.toBeNull()

    view.rerender(<ConversationMessageList {...commonProps} jumpMessageId="" />)
    expect(older.querySelector('[title="Collapse message"]')).not.toBeNull()
  })
})
