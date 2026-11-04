import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { cleanup, fireEvent, render } from '@testing-library/react'
import type { Message } from '../../types'
import { accounts$ } from '../../states/accounts'
import { compose$ } from '../../states/composeState'
import { mail$ } from '../../states/mail'
import { ui$ } from '../../states/ui'
import { QuickReplyRecipients } from './QuickReplyRecipients'

// We were copied on a message to Alice and Bob: a plain reply goes to the
// sender alone, Reply all also copies them.
const thread = (allCc: string, adds = true): Message => ({
  id: 'root',
  account_id: 'acc-1',
  folder_id: 'INBOX',
  thread_id: 't-1',
  from_name: 'Them',
  from_addr: 'them@example.com',
  to: allCc,
  cc: 'me@example.com',
  subject: 'Plans',
  preview: '',
  body: '',
  date: 1000,
  unread: false,
  starred: false,
  has_attachments: false,
  reply: {
    to: 'Them <them@example.com>',
    cc: '',
    all_to: 'Them <them@example.com>',
    all_cc: allCc,
    all_adds_recipients: adds,
  },
})

const open = (message: Message) => {
  mail$.threads.set([message])
  mail$.messages.set([message])
  ui$.selectedThread.set('t-1')
}

describe('QuickReplyRecipients', () => {
  afterEach(() => {
    cleanup()
    compose$.quickReplyAdded.set([])
    mail$.threads.set([])
    mail$.messages.set([])
    ui$.selectedThread.set('')
    accounts$.set([])
  })

  beforeEach(() => {
    compose$.quickReplyAdded.set([])
    ui$.selectedAccount.set('acc-1')
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

  it('lists the people Reply all would add as Cc rows, shown on hover or focus of the line', () => {
    open(thread('Alice <alice@example.com>, Bob <bob@example.com>'))
    const view = render(<QuickReplyRecipients />)

    const list = view.getByRole('list')
    const shown = list.closest('.hidden')?.className ?? ''
    expect(shown).toContain('group-hover:block')
    expect(shown).toContain('group-focus-within:block')
    // The shared menu panel, like every other menu.
    expect(list.closest('.menu-surface')).toBeTruthy()
    const alice = view.getByTitle('Alice <alice@example.com>')
    expect(alice.textContent).toBe('CcAlice')
    expect(view.getByTitle('Bob <bob@example.com>')).toBeTruthy()
  })

  it('adds the one clicked, next to the recipient, and drops it from the list', () => {
    open(thread('Alice <alice@example.com>, Bob <bob@example.com>'))
    const view = render(<QuickReplyRecipients />)

    fireEvent.click(view.getByTitle('Bob <bob@example.com>').querySelector('button')!)

    expect(compose$.quickReplyAdded.get()).toEqual(['bob@example.com'])
    expect(view.queryByTitle('Bob <bob@example.com>')).toBeNull()
    expect(view.getByTitle('Alice <alice@example.com>')).toBeTruthy()
    expect(view.container.textContent).toMatch(/Cc\s*Bob/)
  })

  it('has no list when Reply all reaches no one new', () => {
    open(thread('', false))
    const view = render(<QuickReplyRecipients />)

    expect(view.queryByRole('list')).toBeNull()
  })
})
