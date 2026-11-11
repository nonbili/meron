import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, renderHook } from '@testing-library/react'
// Initialize this side of the mail/compose cycle before the body pulls both
// modules in through its native/link helpers (see quoteFold.test).
import '../../states/compose'
import type { Account, Message } from '../../types'
import { accounts$ } from '../../states/accounts'
import { settings$ } from '../../states/settings'
import { MessageBubbleBody } from './MessageBubbleBody'
import { foldStartWithOwnSignature, markOwnSignature, ownSignatureText, useOwnSignatureText } from './ownSignatureFold'
import { messageMatchCount } from './threadSearchMatches'

const account: Account = {
  id: 'account-1',
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

function message(body: string, outgoing: boolean, quoteStart: number | null = null): Message {
  return {
    id: 'message-1',
    account_id: 'account-1',
    folder_id: outgoing ? 'Sent' : 'INBOX',
    thread_id: 'thread-1',
    from_name: outgoing ? 'Me' : 'Them',
    from_addr: outgoing ? 'me@example.com' : 'them@example.com',
    to: outgoing ? 'them@example.com' : 'me@example.com',
    subject: 'Subject',
    preview: body,
    body,
    body_quote_start: quoteStart,
    outgoing,
    date: 0,
    unread: false,
    starred: false,
    has_attachments: false,
  }
}

const renderBody = (msg: Message, query = '', activeSearchOffset = -1) =>
  render(
    <MessageBubbleBody
      message={msg}
      useHtmlBody={false}
      outgoing={msg.outgoing}
      normalizedSearchQuery={query}
      activeSearchOffset={activeSearchOffset}
    />,
  )

beforeEach(() => {
  accounts$.set([account])
  settings$.signature.set('<p>Me</p><p>Phone 123</p>')
})

afterEach(() => {
  cleanup()
  accounts$.set([])
  settings$.signature.set('')
})

describe('ownSignatureText', () => {
  it('is the sending account’s signature, for my own messages only', () => {
    expect(ownSignatureText(message('x', true))).toBe('Me\nPhone 123')
    expect(ownSignatureText(message('x', false))).toBe('')
  })

  it('counts the bubble shown while my message is still being sent', () => {
    // The optimistic bubble isn't marked outgoing (its Sent copy is), but only my
    // own sends carry a local send status.
    for (const send_status of ['sending', 'sent', 'failed'] as const) {
      expect(ownSignatureText({ ...message('x', false), send_status })).toBe('Me\nPhone 123')
    }
  })

  it('follows a per-account override', () => {
    accounts$.set([{ ...account, signature: { mode: 'none', html: '' } }])
    expect(ownSignatureText(message('x', true))).toBe('')
  })
})

// #154 review, point 5: every bubble in a thread reads this.
describe('useOwnSignatureText', () => {
  it('renders again when my signature changes, not for any other account change', () => {
    const other = { ...account, id: 'account-2', email: 'other@example.com' }
    accounts$.set([account, other])
    let renders = 0
    const { result } = renderHook(() => {
      renders++
      return useOwnSignatureText(message('x', true))
    })
    expect(result.current).toBe('Me\nPhone 123')
    const before = renders

    act(() => accounts$.set([account, { ...other, display_name: 'Renamed' }]))
    act(() => accounts$.set([{ ...account, display_name: 'Me, renamed' }, other]))
    expect(renders).toBe(before)

    act(() => accounts$.set([{ ...account, signature: { mode: 'custom', html: '<p>New</p>' } }, other]))
    expect(result.current).toBe('New')
    act(() => settings$.signature.set('<p>Global</p>'))
    act(() => accounts$.set([account, other]))
    expect(result.current).toBe('Global')
  })
})

describe('foldStartWithOwnSignature', () => {
  const sig = 'Me\nPhone 123'

  it('starts the fold at a signature that ends the message', () => {
    const body = 'Thanks\n\nMe\nPhone 123'
    expect(foldStartWithOwnSignature(body, null, sig)).toBe('Thanks'.length)
  })

  it('folds the signature with the quote under it', () => {
    const body = 'Thanks\n\nMe\nPhone 123\n\nOn Mon, Jane wrote:\n> Lunch?'
    const quoteStart = body.indexOf('On Mon')
    expect(foldStartWithOwnSignature(body, quoteStart, sig)).toBe('Thanks'.length)
  })

  it('keeps the quote fold when the signature is not the last thing above it', () => {
    const body = 'Me\nPhone 123\n\nThanks\n\nOn Mon, Jane wrote:\n> Lunch?'
    const quoteStart = body.indexOf('On Mon')
    expect(foldStartWithOwnSignature(body, quoteStart, sig)).toBe(quoteStart)
    expect(foldStartWithOwnSignature('Thanks\n\nMe\nPhone 123, edited', null, sig)).toBeNull()
  })

  it('folds nothing of a message that is only the signature', () => {
    expect(foldStartWithOwnSignature('Me\nPhone 123', null, sig)).toBeNull()
  })

  it("recognises the full composer's plain form, a blank line between paragraphs", () => {
    const body = 'Thanks\n\nMe\n\nPhone 123'
    expect(foldStartWithOwnSignature(body, null, sig)).toBeNull()
    expect(foldStartWithOwnSignature(body, null, sig, 'Me\n\nPhone 123')).toBe('Thanks'.length)
  })

  it('changes nothing without a signature', () => {
    expect(foldStartWithOwnSignature('Thanks\n\nMe', 7, '')).toBe(7)
  })
})

describe('markOwnSignature', () => {
  const doc = (html: string) => new DOMParser().parseFromString(html, 'text/html')
  const marked = (d: Document) => Array.from(d.querySelectorAll('[data-meron-quote]')).map((el) => el.textContent)

  it('marks the blocks of a signature that ends the message, and the blank line above it', () => {
    const d = doc('<p>Thanks</p><p></p><p>Me</p><p>Phone 123</p>')
    expect(markOwnSignature(d, 'Me\nPhone 123')).toBe(true)
    expect(marked(d)).toEqual(['', 'Me', 'Phone 123'])
  })

  it('reads line breaks inside a block as lines', () => {
    const d = doc('<p>Thanks</p><p>Me<br>Phone 123</p>')
    expect(markOwnSignature(d, 'Me\nPhone 123')).toBe(true)
    expect(marked(d)).toEqual(['MePhone 123'])
  })

  it('looks inside a wrapper around the whole message', () => {
    const d = doc('<div dir="ltr"><p>Thanks</p><p>Me</p><p>Phone 123</p></div>')
    expect(markOwnSignature(d, 'Me\nPhone 123')).toBe(true)
    expect(marked(d)).toEqual(['Me', 'Phone 123'])
  })

  it('marks a signature right above the marked quote', () => {
    const d = doc(
      '<p>Thanks</p><p>Me</p><p>Phone 123</p><div data-meron-quote=""><blockquote>Lunch?</blockquote></div>',
    )
    expect(markOwnSignature(d, 'Me\nPhone 123')).toBe(true)
    expect(marked(d)).toEqual(['Me', 'Phone 123', 'Lunch?'])
  })

  it('leaves a message alone when it does not end in the signature, or is only the signature', () => {
    for (const html of [
      // A picture after the signature is content, not a blank line.
      '<p>Thanks</p><p>Me</p><p>Phone 123</p><img src="photo.jpg">',
      '<p>Thanks</p><p>Me</p><p>Phone 123</p><p>PS: more</p>',
      '<p>Thanks</p><p>Me</p><p>Phone 456</p>',
      '<p>Me</p><p>Phone 123</p>',
    ]) {
      const d = doc(html)
      expect(markOwnSignature(d, 'Me\nPhone 123')).toBe(false)
      expect(marked(d)).toEqual([])
    }
  })
})

describe('MessageBubbleBody with my own signature', () => {
  it('folds my signature behind a toggle that names it', () => {
    const { container, getByRole } = renderBody(message('Thanks (own)\n\nMe\nPhone 123', true))
    expect(container.textContent).toContain('Thanks (own)')
    expect(container.textContent).not.toContain('Phone 123')

    const toggle = getByRole('button', { expanded: false })
    expect(toggle.getAttribute('aria-label')).toBe('Show signature')
    fireEvent.click(toggle)
    expect(container.textContent).toContain('Phone 123')
  })

  it('folds my signature from the first render of a bubble still sending', () => {
    const { container } = renderBody({
      ...message('On its way (sending)\n\nMe\nPhone 123', false),
      send_status: 'sending',
    })
    expect(container.textContent).toContain('On its way (sending)')
    expect(container.textContent).not.toContain('Phone 123')
  })

  it('shows the signature in a message from someone else', () => {
    const { container, queryByRole } = renderBody(message('Thanks (theirs)\n\nMe\nPhone 123', false))
    expect(container.textContent).toContain('Phone 123')
    expect(queryByRole('button', { expanded: false })).toBeNull()
  })

  it('counts the same matches the unfolded body marks', () => {
    const msg = message('phone me (count)\n\nMe\nPhone 123', true)
    const count = messageMatchCount(msg, 'phone', false)
    expect(count).toBe(2)
    // Parked on the match inside the signature: the fold opens to show it.
    const { container } = renderBody(msg, 'phone', count - 1)
    expect(container.querySelectorAll('mark').length).toBe(count)
  })
})
