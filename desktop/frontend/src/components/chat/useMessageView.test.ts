import { afterEach, beforeEach, expect, it } from 'bun:test'
import { cleanup, renderHook } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import type { Message } from '../../types'
import { useMessageView } from './useMessageView'

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
})

it("resolves no sender image for an outgoing message, which shows the account's own avatar", () => {
  expect(renderHook(() => useMessageView(sent)).result.current.avatarEmail).toBeUndefined()
})

it("resolves an incoming message's sender image from its From address", () => {
  const received = { ...sent, id: 'received', outgoing: false, from_name: 'Jacob', from_addr: 'jacob@example.com' }
  expect(renderHook(() => useMessageView(received)).result.current.avatarEmail).toBe('jacob@example.com')
})
