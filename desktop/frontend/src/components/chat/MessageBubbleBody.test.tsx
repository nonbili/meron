import { afterEach, describe, expect, it } from 'bun:test'
import { cleanup, render } from '@testing-library/react'
// Initialize this side of the mail/compose cycle before the body pulls both
// modules in through its native/link helpers (see ConversationMessageList.test).
import '../../states/compose'
import type { Message } from '../../types'
import { MessageBubbleBody } from './MessageBubbleBody'
import { messageMatchCount } from './threadSearchMatches'

function message(body: string): Message {
  return {
    id: 'message-1',
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

function marks(body: string, query: string, activeSearchOffset = -1) {
  const { container } = render(
    <MessageBubbleBody
      message={message(body)}
      useHtmlBody={false}
      normalizedSearchQuery={query}
      activeSearchOffset={activeSearchOffset}
    />,
  )
  return container.querySelectorAll('mark')
}

afterEach(cleanup)

describe('MessageBubbleBody search highlighting', () => {
  // The counter promises a number of stops; each one has to be a mark the user
  // can actually be taken to, whatever the two sides think case folding means.
  it('marks exactly as many matches as the search bar counted', () => {
    for (const [body, query] of [
      ['kitchen sink, kitchen door', 'kitchen'],
      ['Kitchen kitchen KITCHEN', 'kitchen'],
      // "İ".toLowerCase() starts with "i", but /i/i does not match "İ".
      ['İstanbul', 'i'],
      // "ΟΣ" lowercases to a final sigma, which /σ/i matches and "σ" does not.
      ['ΟΣ', 'ος'],
      ['a **bold kitchen** and `code kitchen`', 'kitchen'],
    ] as const) {
      expect(marks(body, query).length).toBe(messageMatchCount(message(body), query, false))
      cleanup()
    }
  })

  it('gives the stronger highlight to the occurrence the search is parked on', () => {
    const found = marks('kitchen sink, kitchen door', 'kitchen', 1)
    expect(found.length).toBe(2)
    expect(found[0].getAttribute('data-search-active')).toBeNull()
    expect(found[1].getAttribute('data-search-active')).toBe('true')
  })
})
