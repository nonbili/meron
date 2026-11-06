import { afterEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { createRef } from 'react'
import type { ReactElement } from 'react'
// Initialize this side of the mail/compose cycle before ConversationMessageList
// pulls both modules in through its message actions and mail paging imports.
import '../../states/compose'
import { settings$ } from '../../states/settings'
import { setAllMessagesExpanded, thread$ } from '../../states/thread'
import { forgetThreadViews } from '../../states/threadView'
import type { Message } from '../../types'
import { ConversationMessageList } from './ConversationMessageList'
import { ConversationScrollContext, type ConversationScroll } from './useConversationScroll'

function withScroll(overrides: Partial<ConversationScroll>, list: ReactElement) {
  const scroll: ConversationScroll = {
    scrollRef: createRef<HTMLDivElement>(),
    messagesWrapperRef: createRef<HTMLDivElement>(),
    bottomAnchorRef: createRef<HTMLDivElement>(),
    handleConversationScroll: () => undefined,
    maybeMarkRead: () => undefined,
    setScrollTop: () => undefined,
    scrollMessageToTop: () => undefined,
    holdTopMessage: () => undefined,
    releasePinForUserScroll: () => undefined,
    ...overrides,
  }
  return <ConversationScrollContext value={scroll}>{list}</ConversationScrollContext>
}

function message(id: string, body: string, threadId = 'thread-1'): Message {
  return {
    id,
    account_id: 'account-1',
    folder_id: 'inbox',
    thread_id: threadId,
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
  thread$.expandAll.set(null)
  forgetThreadViews()
})

describe('ConversationMessageList direct jumps', () => {
  it('keeps an older jumped-to message expanded after its highlight ends', () => {
    settings$.conversationLayout.set('traditional')
    const messages = [message('older', 'Older body'), message('newer', 'Newer body')]
    const scroll = {
      scrollRef: createRef<HTMLDivElement>(),
      messagesWrapperRef: createRef<HTMLDivElement>(),
      bottomAnchorRef: createRef<HTMLDivElement>(),
    }
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
      wallpaperClassName: '',
    }

    const view = render(withScroll(scroll, <ConversationMessageList {...commonProps} jumpMessageId="" />))
    const older = view.container.querySelector<HTMLElement>('[data-message-id="older"]')!
    expect(older.querySelector('[title="Expand message"]')).not.toBeNull()

    view.rerender(withScroll(scroll, <ConversationMessageList {...commonProps} jumpMessageId="older" />))
    expect(older.querySelector('[title="Collapse message"]')).not.toBeNull()

    view.rerender(withScroll(scroll, <ConversationMessageList {...commonProps} jumpMessageId="" />))
    expect(older.querySelector('[title="Collapse message"]')).not.toBeNull()
  })

  it('scrolls an expanded message only after its full row has committed', () => {
    settings$.conversationLayout.set('traditional')
    const messages = [message('older', 'Older body'), message('newer', 'Newer body')]
    let expandedWhenScrolled = false
    const view = render(
      withScroll(
        {
          scrollMessageToTop: (messageId) => {
            expandedWhenScrolled =
              messageId === 'older' &&
              view.container.querySelector('[data-message-id="older"] [title="Collapse message"]') !== null
          },
        },
        <ConversationMessageList
          messages={messages}
          showThreadLoading={false}
          showThreadError={false}
          onRetryThreadLoad={() => undefined}
          messagesCursor=""
          messagesLoadingMore={false}
          activeThreadId="thread-1"
          searchMatches={[]}
          activeSearchId=""
          jumpMessageId=""
          galleryOffsets={new Map<string, number>()}
          wallpaperClassName=""
        />,
      ),
    )

    fireEvent.click(view.container.querySelector('[data-message-id="older"] [title="Expand message"]')!)

    expect(expandedWhenScrolled).toBe(true)
  })

  it('reports wheel input separately from the resulting scroll event', () => {
    let scrollIntents = 0
    let scrollEvents = 0
    const view = render(
      withScroll(
        {
          handleConversationScroll: () => scrollEvents++,
          releasePinForUserScroll: () => scrollIntents++,
        },
        <ConversationMessageList
          messages={[message('newer', 'Newer body')]}
          showThreadLoading={false}
          showThreadError={false}
          onRetryThreadLoad={() => undefined}
          messagesCursor=""
          messagesLoadingMore={false}
          activeThreadId="thread-1"
          searchMatches={[]}
          activeSearchId=""
          jumpMessageId=""
          galleryOffsets={new Map<string, number>()}
          wallpaperClassName=""
        />,
      ),
    )

    const scroller = view.container.querySelector('.message-scroll')!
    fireEvent.scroll(scroller)
    expect(scrollIntents).toBe(0)
    expect(scrollEvents).toBe(1)

    fireEvent.wheel(scroller)
    expect(scrollIntents).toBe(1)
  })
})

describe('ConversationMessageList expand all', () => {
  const list = (messages: Message[], threadId = 'thread-1') => (
    <ConversationMessageList
      messages={messages}
      showThreadLoading={false}
      showThreadError={false}
      onRetryThreadLoad={() => undefined}
      messagesCursor=""
      messagesLoadingMore={false}
      activeThreadId={threadId}
      searchMatches={[]}
      activeSearchId=""
      jumpMessageId=""
      galleryOffsets={new Map<string, number>()}
      wallpaperClassName=""
    />
  )
  const collapsed = (container: HTMLElement) =>
    Array.from(container.querySelectorAll<HTMLElement>('[data-message-id]'))
      .filter((element) => element.querySelector('[title="Expand message"]'))
      .map((element) => element.dataset.messageId)

  it('expands every message, then collapses all but the newest', () => {
    settings$.conversationLayout.set('traditional')
    const messages = [message('first', 'First'), message('second', 'Second'), message('newest', 'Newest')]
    const held: boolean[] = []
    const view = render(withScroll({ holdTopMessage: (collapsing) => held.push(collapsing) }, list(messages)))
    expect(collapsed(view.container)).toEqual(['first', 'second'])
    expect(thread$.expandAllAction.peek()).toBe('expand')

    act(() => setAllMessagesExpanded(true))
    expect(collapsed(view.container)).toEqual([])
    expect(thread$.expandAllAction.peek()).toBe('collapse')

    act(() => setAllMessagesExpanded(false))
    expect(collapsed(view.container)).toEqual(['first', 'second'])
    expect(held).toEqual([false, true])
  })

  it('overrides messages toggled one by one, and applies to ones loaded later', () => {
    settings$.conversationLayout.set('traditional')
    const messages = [message('first', 'First'), message('second', 'Second'), message('newest', 'Newest')]
    const view = render(withScroll({}, list(messages)))
    fireEvent.click(view.container.querySelector('[data-message-id="newest"] [title="Collapse message"]')!)
    expect(collapsed(view.container)).toEqual(['first', 'second', 'newest'])

    act(() => setAllMessagesExpanded(true))
    expect(collapsed(view.container)).toEqual([])

    // Asking again for what is already in force still resets a later toggle.
    fireEvent.click(view.container.querySelector('[data-message-id="first"] [title="Collapse message"]')!)
    expect(collapsed(view.container)).toEqual(['first'])
    act(() => setAllMessagesExpanded(true))
    expect(collapsed(view.container)).toEqual([])

    view.rerender(withScroll({}, list([message('earlier', 'Earlier'), ...messages])))
    expect(collapsed(view.container)).toEqual([])
  })

  it('keeps what was opened and closed when leaving the thread and coming back', () => {
    settings$.conversationLayout.set('traditional')
    const messages = [
      { ...message('first', 'First'), unread: true },
      message('second', 'Second'),
      message('newest', 'Newest'),
    ]
    const other = [message('a', 'A', 'thread-2'), message('b', 'B', 'thread-2')]
    const view = render(withScroll({}, list(messages)))
    expect(collapsed(view.container)).toEqual(['second'])
    fireEvent.click(view.container.querySelector('[data-message-id="second"] [title="Expand message"]')!)
    fireEvent.click(view.container.querySelector('[data-message-id="newest"] [title="Collapse message"]')!)
    expect(collapsed(view.container)).toEqual(['newest'])

    view.rerender(withScroll({}, list(other, 'thread-2')))
    expect(collapsed(view.container)).toEqual(['a'])
    act(() => setAllMessagesExpanded(true))
    expect(collapsed(view.container)).toEqual([])

    // Back in the first thread, read by now and with the shared expand-all
    // reset as it is on every switch.
    act(() => thread$.expandAll.set(null))
    view.rerender(withScroll({}, list(messages.map((m) => ({ ...m, unread: false })))))
    expect(collapsed(view.container)).toEqual(['newest'])

    view.rerender(withScroll({}, list(other, 'thread-2')))
    expect(collapsed(view.container)).toEqual([])
  })

  it('keeps them when the conversation pane mounts again', () => {
    settings$.conversationLayout.set('traditional')
    const messages = [message('first', 'First'), message('second', 'Second'), message('newest', 'Newest')]
    const view = render(withScroll({}, list(messages)))
    fireEvent.click(view.container.querySelector('[data-message-id="first"] [title="Expand message"]')!)
    expect(collapsed(view.container)).toEqual(['second'])
    view.unmount()

    const again = render(withScroll({}, list(messages)))
    expect(collapsed(again.container)).toEqual(['second'])
  })

  it('offers nothing in the chat layout or for a single message', () => {
    const messages = [message('first', 'First'), message('newest', 'Newest')]
    const view = render(withScroll({}, list(messages)))
    expect(thread$.expandAllAction.peek()).toBeNull()

    act(() => settings$.conversationLayout.set('traditional'))
    view.rerender(withScroll({}, list([messages[1]])))
    expect(thread$.expandAllAction.peek()).toBeNull()

    // Collapsed by hand, the one message can still be expanded from the menu.
    fireEvent.click(view.container.querySelector('[title="Collapse message"]')!)
    expect(thread$.expandAllAction.peek()).toBe('expand')
  })

  it('opens the thread menu for a right-click off the messages only', () => {
    const view = render(withScroll({}, list([message('newest', 'Newest')])))
    fireEvent.contextMenu(view.container.querySelector('.message-scroll')!, { clientX: 12, clientY: 34 })
    expect(thread$.conversationMenu.peek()).toEqual({ x: 12, y: 34 })

    thread$.conversationMenu.set(null)
    fireEvent.contextMenu(view.container.querySelector('[data-message-id="newest"]')!)
    expect(thread$.conversationMenu.peek()).toBeNull()
    thread$.messageContextMenu.set(null)
  })
})
