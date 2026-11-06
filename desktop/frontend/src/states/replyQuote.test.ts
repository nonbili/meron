import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { MessageCrypto } from '../lib/crypto'
import type { Message } from '../types'
import { replyQuote } from './compose'
import { settings$ } from './settings'

const crypto = (overrides: Partial<MessageCrypto> = {}): MessageCrypto => ({
  encrypted: true,
  signed: true,
  trust: 'known',
  signer: 'C0FFEE',
  partial: false,
  weak_hash: false,
  key_changed: false,
  verified_conflict: false,
  on_list: false,
  own: false,
  signer_stored: true,
  offer: false,
  ...overrides,
})

const message = (overrides: Partial<Message> = {}): Message => ({
  id: 'm1',
  account_id: 'acc',
  folder_id: 'inbox',
  thread_id: 't1',
  from_name: 'Mallory',
  from_addr: 'mallory@example.com',
  to: 'me@example.com',
  subject: 'Topic',
  preview: '',
  body: 'the secret',
  date: 100,
  unread: false,
  starred: false,
  has_attachments: false,
  ...overrides,
})

// Audit H1 ("Re: What's up Johnny?"): an attacker can resend a ciphertext once
// sent to us from their own address. Our reply would be encrypted to them, so
// "only quote into encrypted replies" doesn't help: the quote itself must stay
// out unless the sender is shown to have written it.
describe('replyQuote and decrypted mail', () => {
  let before: boolean
  beforeEach(() => {
    before = settings$.quoteReplies.peek()
    settings$.quoteReplies.set(true)
  })
  afterEach(() => settings$.quoteReplies.set(before))

  it('quotes plain mail and encrypted mail signed by the sender', () => {
    expect(replyQuote(message({ crypto: null }))?.text).toContain('> the secret')
    expect(replyQuote(message({ crypto: crypto() }))?.text).toContain('> the secret')
    expect(replyQuote(message({ crypto: crypto({ trust: 'verified', own: true }) }))?.text).toContain('> the secret')
  })

  it('never quotes a decrypted message the sender is not shown to have written', () => {
    // Unsigned: a replayed ciphertext looks exactly like this.
    expect(
      replyQuote(message({ crypto: crypto({ signed: false, signer: null, trust: 'unconfirmed' }) })),
    ).toBeUndefined()
    // Signed by someone other than From: the replay of a signed message.
    expect(
      replyQuote(message({ crypto: crypto({ trust: 'unconfirmed', problem: 'signer_mismatch' }) })),
    ).toBeUndefined()
    // Signed by a key we don't know (no Autocrypt, nothing stored).
    expect(
      replyQuote(message({ crypto: crypto({ signer: null, issuer: 'ABCD', trust: 'unconfirmed' }) })),
    ).toBeUndefined()
  })
})
