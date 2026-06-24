import { describe, expect, it } from 'bun:test'
import {
  discardPendingSend,
  getPendingSend,
  isLocalSendId,
  LOCAL_SEND_PREFIX,
  setPendingSend,
  type PendingSend,
} from './pendingSends'

const payload: PendingSend = {
  account_id: 'acct-1',
  to: 'to@example.com',
  cc: '',
  subject: 'Hello',
  body: 'Body',
  html: '<p>Body</p>',
  in_reply_to: '',
  references: '',
  from: 'me@example.com',
  message_id: '<local@example.com>',
  attachments: [{ filename: 'a.txt', mime: 'text/plain', data: 'YQ==', inline_id: '' }],
}

describe('pending sends registry', () => {
  it('recognizes local optimistic send ids', () => {
    expect(LOCAL_SEND_PREFIX).toBe('sent-')
    expect(isLocalSendId('sent-123')).toBe(true)
    expect(isLocalSendId('imap:acct:INBOX:123')).toBe(false)
  })

  it('stores, replaces, and discards pending send payloads', () => {
    const id = 'sent-test-registry'
    discardPendingSend(id)

    setPendingSend(id, payload)
    expect(getPendingSend(id)).toEqual(payload)

    const replacement = { ...payload, subject: 'Updated' }
    setPendingSend(id, replacement)
    expect(getPendingSend(id)).toEqual(replacement)

    discardPendingSend(id)
    expect(getPendingSend(id)).toBeUndefined()
  })
})
