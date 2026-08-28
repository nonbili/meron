import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { cleanup, fireEvent, render } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { mail$ } from '../../states/mail'
import { ui$ } from '../../states/ui'
import type { Message } from '../../types'
import { MessageBubble } from './MessageBubble'

const sent: Message = {
  id: 'sent',
  account_id: 'acc-1',
  folder_id: 'Sent',
  thread_id: 'thread-1',
  from_name: 'Me',
  from_addr: 'me@example.com',
  to: 'Jacob <jacob@example.com>',
  subject: 'Subject',
  preview: 'Body',
  body: 'Body',
  date: 0,
  outgoing: true,
  unread: false,
  starred: false,
  has_attachments: false,
}

function openDetails(message: Message) {
  const view = render(<MessageBubble message={message} galleryOffset={0} onOpenContextMenu={() => undefined} />)
  fireEvent.click(view.getByTitle('Show details'))
  return view.getByText('From:').parentElement?.parentElement
}

describe('MessageBubble details', () => {
  beforeEach(() => {
    accounts$.set([
      {
        id: 'acc-1',
        email: 'me@example.com',
        display_name: 'Me',
        provider: 'custom',
        auth_type: 'password',
        imap_host: 'imap.example.com',
        imap_port: 993,
        smtp_host: 'smtp.example.com',
        smtp_port: 465,
        tls: true,
      },
    ])
  })

  afterEach(() => {
    cleanup()
    accounts$.set([])
    mail$.threads.set([])
    ui$.selectedThread.set('')
  })

  it('keeps an outgoing details popup at its own width when the bubble is plain text', () => {
    // A plain-text bubble shrinks to its content, so clamping to it would
    // squeeze the addresses down to a couple of characters per line.
    expect(openDetails(sent)?.classList.contains('max-w-full')).toBe(false)
  })

  it('constrains an outgoing details popup to an html bubble, which is a fixed share of the pane', () => {
    mail$.threads.set([sent])
    ui$.selectedThread.set('thread-1')

    expect(openDetails({ ...sent, body_html: '<p>Body</p>' })?.classList.contains('max-w-full')).toBe(true)
  })
})
