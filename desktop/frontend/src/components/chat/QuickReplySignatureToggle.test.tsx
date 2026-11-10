import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import type { Account, Message } from '../../types'
import { accounts$ } from '../../states/accounts'
import { compose$ } from '../../states/composeState'
import { mail$ } from '../../states/mail'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import { QuickReplySignatureToggle } from './QuickReplySignatureToggle'

const account: Account = {
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
}

const thread: Message = {
  id: 'root',
  account_id: 'acc-1',
  folder_id: 'INBOX',
  thread_id: 't-1',
  from_name: 'Them',
  from_addr: 'them@example.com',
  to: 'me@example.com',
  subject: 'Plans',
  preview: '',
  body: '',
  date: 1000,
  unread: false,
  starred: false,
  has_attachments: false,
}

const reset = () => {
  cleanup()
  compose$.composer.set('')
  compose$.quickReplySignatureOff.set(false)
  mail$.threads.set([])
  mail$.messages.set([])
  ui$.selectedThread.set('')
  accounts$.set([])
  settings$.signature.set('')
}

describe('QuickReplySignatureToggle', () => {
  beforeEach(() => {
    reset()
    ui$.selectedAccount.set('acc-1')
    accounts$.set([account])
    settings$.signature.set('<p>Me</p><p>Phone</p>')
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
  })

  afterEach(reset)

  it('shows that the signature will be added, with the signature in its tooltip', () => {
    const { getByRole } = render(<QuickReplySignatureToggle />)
    const button = getByRole('switch')

    expect(button.getAttribute('aria-checked')).toBe('true')
    expect(button.getAttribute('title')).toContain('Me\nPhone')
  })

  it('leaves the signature off for this reply on click, and back on with another', () => {
    const { getByRole } = render(<QuickReplySignatureToggle />)

    fireEvent.click(getByRole('switch'))
    expect(compose$.quickReplySignatureOff.get()).toBe(true)
    expect(getByRole('switch').getAttribute('aria-checked')).toBe('false')

    fireEvent.click(getByRole('switch'))
    expect(compose$.quickReplySignatureOff.get()).toBe(false)
  })

  it('is not there when the account sends no signature', () => {
    accounts$.set([{ ...account, signature: { mode: 'none', html: '' } }])
    const { queryByRole } = render(<QuickReplySignatureToggle />)

    expect(queryByRole('switch')).toBeNull()
  })

  it('appears once the app signature has loaded', () => {
    settings$.signature.set('')
    const { queryByRole } = render(<QuickReplySignatureToggle />)
    expect(queryByRole('switch')).toBeNull()

    act(() => settings$.signature.set('<p>Me</p>'))
    expect(queryByRole('switch')).not.toBeNull()
  })
})
