import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor, within } from '@testing-library/react'
import type { MessageCrypto, RecipientKeys } from '../../lib/crypto'
import { confirmKeyChanges, crypto$ } from '../../states/crypto'
import { settleConfirm, ui$ } from '../../states/ui'
import { settings$ } from '../../states/settings'
import { CryptoBadge } from './CryptoBadge'
import { CryptoStrip } from './CryptoStrip'
import { EncryptToggle, encryptionBlocksSend, recipientAddresses } from './EncryptToggle'
import { VerifyKeyDialog } from './VerifyKeyDialog'
import { KeyLookupDialog } from './KeyLookupDialog'
import { KeyChangeNote } from './KeyChangeNote'
import { EnableEncryptionDialog } from './EnableEncryptionDialog'
import { SignToggle } from './SignToggle'

const FPR = '8E9BCE8647564E257A411B71969BC97D064D88D5'
const base: MessageCrypto = {
  encrypted: false,
  signed: false,
  trust: 'unconfirmed',
  partial: false,
  weak_hash: false,
  key_changed: false,
  verified_conflict: false,
  on_list: false,
  own: false,
  signer_stored: false,
  offer: false,
}

let calls: { command: string; payload: any }[]
beforeEach(() => {
  calls = []
  // Another file can leave a confirm open; tests here read ui$.confirm.
  settleConfirm(false)
  crypto$.verify.set(null)
  crypto$.enableFor.set(null)
  crypto$.lookup.set(null)
  crypto$.dismissedNotices.set({})
  settings$.encryptWhenPossible.set(true)
  ui$.selectedThread.set('')
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push({ command, payload })
          if (command === 'pgp.account.state')
            return { enabled: true, key: { fingerprint: 'AAAA'.repeat(10) }, previous: [] }
          return { ok: true }
        },
      },
    },
  }
})
afterEach(() => cleanup())

const badge = (crypto: MessageCrypto | null) =>
  render(<CryptoBadge crypto={crypto} accountId="acc" senderName="Carol" senderAddress="carol@example.com" />)

describe('CryptoBadge', () => {
  test('ordinary mail shows nothing', () => {
    expect(badge(null).container.innerHTML).toBe('')
    expect(badge({ ...base, offer: true }).container.innerHTML).toBe('')
  })

  test('encrypted and verified: an emerald lock with the full state as its label', () => {
    const { getByRole } = badge({ ...base, encrypted: true, signed: true, trust: 'verified', signer: FPR })
    const button = getByRole('button')
    expect(button.getAttribute('data-tone')).toBe('verified')
    expect(button.getAttribute('aria-label')).toContain('End-to-end encrypted')
    expect(button.getAttribute('aria-label')).toContain('which you verified')
  })

  test('signed only with an unknown key: amber, and no verify action', () => {
    const { getByRole, queryByText } = badge({ ...base, signed: true, issuer: FPR })
    const button = getByRole('button')
    expect(button.getAttribute('data-tone')).toBe('unconfirmed')
    fireEvent.click(button)
    expect(queryByText(/Verify Carol/)).toBeNull()
  })

  test('a known key offers to verify it and opens the dialog for that fingerprint', () => {
    const { getByRole, getByText } = badge({
      ...base,
      encrypted: true,
      signed: true,
      trust: 'known',
      signer: FPR,
      signer_stored: true,
    })
    fireEvent.click(getByRole('button'))
    fireEvent.click(getByText("Verify Carol's key…"))
    expect(crypto$.verify.get()).toEqual({
      addr: 'carol@example.com',
      name: 'Carol',
      fingerprint: FPR,
      accountId: 'acc',
    })
  })

  test('a weak identity binding is amber and explains the policy rejection', () => {
    const { getByRole, getByText, queryByText } = badge({
      ...base,
      signed: true,
      signer: FPR,
      problem: 'weak_key_binding',
    })
    const button = getByRole('button')
    expect(button.getAttribute('data-tone')).toBe('unconfirmed')
    expect(button.getAttribute('aria-label')).toContain('weak hash to bind its identity')
    fireEvent.click(button)
    expect(getByText(/The sender’s identity can’t be confirmed/)).toBeTruthy()
    expect(queryByText(/isn't Carol's/)).toBeNull()
  })

  test('a verified-key conflict takes precedence over a weak binding in the details', () => {
    const { getByRole, getByText, queryByText } = badge({
      ...base,
      signed: true,
      signer: FPR,
      problem: 'weak_key_binding',
      key_changed: true,
      verified_conflict: true,
    })
    const button = getByRole('button')
    expect(button.getAttribute('data-tone')).toBe('danger')
    expect(button.getAttribute('aria-label')).toContain('different key than the one you verified for Carol')
    fireEvent.click(button)
    expect(getByText(/different key than the one you verified for Carol/)).toBeTruthy()
    expect(queryByText(/weak hash to bind its identity/)).toBeNull()
  })

  test('a bad signature is rose', () => {
    expect(
      badge({ ...base, signed: true, problem: 'invalid_signature' })
        .getByRole('button')
        .getAttribute('data-tone'),
    ).toBe('danger')
  })
})

describe('CryptoStrip', () => {
  const strip = (crypto: MessageCrypto, messageId = 'message') =>
    render(
      <CryptoStrip
        messageId={messageId}
        crypto={crypto}
        accountId="acc"
        accountAddress="me@example.org"
        senderName="Carol"
        senderAddress="carol@example.com"
      />,
    )

  test('a weak identity binding shows a calm amber notice', () => {
    const { getByRole } = strip({ ...base, signed: true, problem: 'weak_key_binding' })
    expect(getByRole('status').textContent).toContain('Sender identity couldn’t be verified')
    expect(getByRole('status').className).toContain('bg-amber-500/10')
    expect(getByRole('status').getAttribute('data-danger')).toBeNull()
    expect(getByRole('button', { name: 'Close' })).toBeTruthy()
  })

  test('dismissal follows only that message across remounts, without changing sender trust', () => {
    const crypto = { ...base, signed: true, signer: FPR, problem: 'weak_key_binding' as const }
    const first = strip(crypto, 'first')
    const second = strip(crypto, 'second')
    const before = calls.length
    fireEvent.click(within(first.container).getByRole('button', { name: 'Close' }))
    expect(first.container.innerHTML).toBe('')
    expect(within(second.container).getByRole('status')).toBeTruthy()
    expect(calls).toHaveLength(before)
    first.unmount()
    expect(strip(crypto, 'first').container.innerHTML).toBe('')
    expect(crypto.problem).toBe('weak_key_binding')
    expect(crypto.trust).toBe('unconfirmed')
  })

  test('the signature indicator still opens the technical details after dismissal', () => {
    const crypto = { ...base, signed: true, signer: FPR, problem: 'weak_key_binding' as const }
    const notice = strip(crypto)
    const indicator = badge(crypto)
    fireEvent.click(within(notice.container).getByRole('button', { name: 'Close' }))
    const button = within(indicator.container).getByRole('button')
    expect(button.getAttribute('data-tone')).toBe('unconfirmed')
    fireEvent.click(button)
    expect(indicator.getByText(/weak hash to bind its identity/)).toBeTruthy()
  })

  test('dismissal cannot suppress a new signing key or a stronger verification problem', () => {
    const crypto = { ...base, signed: true, signer: FPR, problem: 'weak_key_binding' as const }
    const props = {
      messageId: 'message',
      accountId: 'acc',
      accountAddress: 'me@example.org',
      senderName: 'Carol',
      senderAddress: 'carol@example.com',
    }
    const view = render(<CryptoStrip {...props} crypto={crypto} />)
    fireEvent.click(view.getByRole('button', { name: 'Close' }))
    view.rerender(<CryptoStrip {...props} crypto={{ ...crypto, signer: 'NEW-SIGNING-KEY' }} />)
    expect(view.getByRole('status')).toBeTruthy()
    view.rerender(<CryptoStrip {...props} crypto={{ ...crypto, problem: 'invalid_signature' }} />)
    expect(view.getByRole('alert')).toBeTruthy()
    expect(view.queryByRole('button', { name: 'Close' })).toBeNull()
    view.rerender(<CryptoStrip {...props} crypto={{ ...crypto, verified_conflict: true }} />)
    expect(view.getByRole('alert').textContent).toContain('different key than the one you verified for Carol')
    expect(view.queryByRole('button', { name: 'Close' })).toBeNull()
    expect(view.queryByRole('button', { name: 'Verify the new key…' })).toBeNull()
  })

  test('an unverified key change stays visible and cannot be dismissed even with a weak binding', () => {
    const { getByRole, queryByRole } = strip({
      ...base,
      signed: true,
      signer: FPR,
      problem: 'weak_key_binding',
      key_changed: true,
    })
    expect(getByRole('status').textContent).toContain("Carol's key changed")
    expect(queryByRole('button', { name: 'Close' })).toBeNull()
    expect(queryByRole('button', { name: 'Verify…' })).toBeNull()
    expect(crypto$.verify.peek()).toBeNull()
  })

  test('a stored changed key still offers a usable verification action', () => {
    const { getByRole } = strip({
      ...base,
      signed: true,
      signer: FPR,
      signer_stored: true,
      key_changed: true,
      verified_conflict: true,
    })
    fireEvent.click(getByRole('button', { name: 'Verify the new key…' }))
    expect(crypto$.verify.peek()).toEqual({
      addr: 'carol@example.com',
      name: 'Carol',
      fingerprint: FPR,
      accountId: 'acc',
    })
  })

  test('a failed decryption is an alert with a way to bring the key', () => {
    const { getByRole, getByText } = strip({ ...base, encrypted: true, problem: 'no_key' })
    expect(getByRole('alert').textContent).toContain("a key Meron doesn't have")
    fireEvent.click(getByText('Use an existing key…'))
    expect(crypto$.enableFor.get()).toEqual({ accountId: 'acc', address: 'me@example.org' })
  })

  test('the offer is a quiet status with turn on, not now and never', async () => {
    const { getByRole, getByText } = strip({ ...base, offer: true })
    expect(getByRole('status').textContent).toContain('Carol uses end-to-end encryption')
    await act(async () => fireEvent.click(getByText('Not now')))
    expect(calls).toContainEqual({
      command: 'pgp.offer.dismiss',
      payload: { account: 'acc', addr: 'carol@example.com' },
    })
    await act(async () => fireEvent.click(getByText("Don't ask again")))
    expect(calls).toContainEqual({ command: 'pgp.offer.dismiss', payload: { account: 'acc', addr: '*' } })
  })

  test('a new key for a verified contact is an alert', () => {
    const { getByRole } = strip({
      ...base,
      encrypted: true,
      signed: true,
      key_changed: true,
      verified_conflict: true,
      signer: FPR,
    })
    expect(getByRole('alert').textContent).toContain('different key than the one you verified')
  })

  test('nothing to say, nothing shown', () => {
    expect(strip({ ...base, encrypted: true, signed: true, trust: 'known' }).container.innerHTML).toBe('')
  })
})

describe('VerifyKeyDialog', () => {
  const open = () => {
    crypto$.verify.set({ addr: 'carol@example.com', name: 'Carol', fingerprint: FPR, accountId: 'acc' })
    return render(<VerifyKeyDialog />)
  }

  test('confirming needs a matching paste or the ticked checkbox', async () => {
    const { getByText, getByLabelText, getByRole } = open()
    const confirm = getByText('Mark as verified').closest('button')!
    expect(confirm.disabled).toBe(true)
    fireEvent.change(getByLabelText(/paste/i), {
      target: { value: '8e9b ce86 4756 4e25 7a41 1b71 969b c97d 064d 88d5' },
    })
    expect(getByRole('status').textContent).toBe('Matches')
    expect(confirm.disabled).toBe(false)
    await act(async () => fireEvent.click(confirm))
    await waitFor(() => expect(calls.some((c) => c.command === 'pgp.peers.setVerified')).toBe(true))
    const call = calls.find((c) => c.command === 'pgp.peers.setVerified')!
    expect(call.payload).toMatchObject({ addr: 'carol@example.com', fingerprint: FPR, verified: true })
  })

  test('a wrong paste blocks verification, even with the checkbox', () => {
    const { getByText, getByLabelText, getByRole } = open()
    fireEvent.change(getByLabelText(/paste/i), { target: { value: '0000 1111 2222 3333 4444' } })
    expect(getByRole('alert').textContent).toContain("Doesn't match")
    expect((getByText('Mark as verified').closest('button') as HTMLButtonElement).disabled).toBe(true)
  })

  test('pasting your own fingerprint says so', async () => {
    const { getByLabelText, findByText } = open()
    // The account's own fingerprint comes from pgp.account.state (stubbed as AAAA×10).
    await findByText('Your fingerprint')
    fireEvent.change(getByLabelText(/paste/i), { target: { value: 'AAAA '.repeat(10) } })
    expect(await findByText(/That's your own fingerprint/)).toBeTruthy()
  })

  test('reading aloud: the checkbox alone enables it', () => {
    const { getByText, getByLabelText } = open()
    fireEvent.click(getByLabelText('I compared them, and they match'))
    expect((getByText('Mark as verified').closest('button') as HTMLButtonElement).disabled).toBe(false)
  })
})

describe('EncryptToggle', () => {
  const keys = (over: Partial<RecipientKeys> = {}): RecipientKeys => ({
    enabled: true,
    own_key_unusable: false,
    missing: [],
    ...over,
  })

  test('hidden when the account has no encryption', () => {
    expect(
      render(<EncryptToggle keys={keys({ enabled: false })} on={false} onChange={() => {}} />).container.innerHTML,
    ).toBe('')
  })

  test('turns on, and reports its state as a switch', () => {
    let value = false
    const { getByRole } = render(<EncryptToggle keys={keys()} on={false} onChange={(on) => (value = on)} />)
    const sw = getByRole('switch')
    expect(sw.getAttribute('aria-checked')).toBe('false')
    fireEvent.click(sw)
    expect(value).toBe(true)
  })

  test('with a recipient lacking a key, the click looks the key up instead of turning on', () => {
    let value = false
    const { getByRole } = render(
      <EncryptToggle
        keys={keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })}
        on={false}
        onChange={(on) => (value = on)}
      />,
    )
    const sw = getByRole('switch') as HTMLButtonElement
    expect(sw.title).toContain('bob@example.net')
    fireEvent.click(sw)
    expect(value).toBe(false)
    expect(crypto$.lookup.get()).toEqual({ addrs: ['bob@example.net'] })
  })

  test('picking a key in the lookup it opened turns encryption on', () => {
    let value = false
    const missing = keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })
    const { getByRole, rerender } = render(<EncryptToggle keys={missing} on={false} onChange={(on) => (value = on)} />)
    fireEvent.click(getByRole('switch'))
    expect(value).toBe(false)
    // The key check refreshes after "Use this key": nobody is missing a key now.
    rerender(<EncryptToggle keys={keys()} on={false} onChange={(on) => (value = on)} />)
    expect(value).toBe(true)
  })

  test('the open lock is the same grey as the composer icons beside it, with or without keys', () => {
    // The attachment and full-editor buttons are text-secondary, hover text-primary;
    // a dimmed lock looked paler than them in light mode.
    const missing = keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })
    for (const k of [missing, keys()]) {
      const { getByRole, unmount } = render(<EncryptToggle keys={k} on={false} onChange={() => {}} />)
      const classes = getByRole('switch').className.split(' ')
      expect(classes).toContain('text-secondary')
      expect(classes).toContain('hover:text-primary')
      expect(classes.some((c) => c.startsWith('text-secondary/'))).toBe(false)
      unmount()
    }
  })

  // A reopened encrypted draft: a recipient without a key doesn't switch it
  // off; the lock looks the key up, then switching off asks, naming the risk.
  test('an encrypted draft is switched off only on purpose', async () => {
    let value = true
    const missing = keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })
    const { getByRole, rerender } = render(
      <EncryptToggle keys={missing} on encryptedDraft onChange={(on) => (value = on)} />,
    )
    expect(value).toBe(true)
    act(() => fireEvent.click(getByRole('switch')))
    expect(crypto$.lookup.get()).toEqual({ addrs: ['bob@example.net'] })
    rerender(<EncryptToggle keys={missing} on encryptedDraft onChange={(on) => (value = on)} />)
    act(() => fireEvent.click(getByRole('switch')))
    await waitFor(() => expect(ui$.confirm.get()?.message).toContain('This draft was saved encrypted'))
    act(() => settleConfirm(true))
    await waitFor(() => expect(value).toBe(false))
  })

  // From an account without encryption nothing can be encrypted: a reply
  // switches off (it goes out without the decrypted quote) rather than leaving
  // Send blocked behind a lock that can't be clicked; a forward, which is
  // decrypted text itself, stays blocked.
  test('an account without encryption switches a reply off but not a forward', () => {
    let value = true
    const off = keys({ enabled: false })
    render(<EncryptToggle keys={off} on sensitive="reply" onChange={(on) => (value = on)} />)
    expect(value).toBe(false)
    cleanup()
    value = true
    render(<EncryptToggle keys={off} on sensitive="forward" onChange={(on) => (value = on)} />)
    expect(value).toBe(true)
    cleanup()
    value = true
    render(<EncryptToggle keys={off} on onChange={(on) => (value = on)} />)
    expect(value).toBe(false)
  })

  // Switched off by the user before (in the quick reply it came from): the
  // preference doesn't turn it back on, though every recipient has a key.
  test('a lock the user switched off stays off', () => {
    let value = false
    render(<EncryptToggle keys={keys()} on={false} offByUser onChange={(on) => (value = on)} />)
    expect(value).toBe(false)
    cleanup()
    render(<EncryptToggle keys={keys()} on={false} onChange={(on) => (value = on)} />)
    expect(value).toBe(true)
  })

  test('with the preference off, a key appearing on its own leaves encryption off', () => {
    settings$.encryptWhenPossible.set(false)
    let value = false
    const { rerender } = render(
      <EncryptToggle
        keys={keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })}
        on={false}
        onChange={(on) => (value = on)}
      />,
    )
    rerender(<EncryptToggle keys={keys()} on={false} onChange={(on) => (value = on)} />)
    expect(value).toBe(false)
  })

  test('encrypts by default when every recipient has a key', () => {
    let value = false
    render(<EncryptToggle keys={keys()} on={false} onChange={(on) => (value = on)} />)
    expect(value).toBe(true)
  })

  test('not without recipients', () => {
    let value = false
    render(<EncryptToggle keys={keys()} on={false} hasRecipients={false} onChange={(on) => (value = on)} />)
    expect(value).toBe(false)
  })

  test('switched off by hand, it stays off for this message', () => {
    let on = true
    const { getByRole, rerender } = render(<EncryptToggle keys={keys()} on onChange={(v) => (on = v)} />)
    act(() => fireEvent.click(getByRole('switch')))
    expect(on).toBe(false)
    rerender(<EncryptToggle keys={keys()} on={false} onChange={(v) => (on = v)} />)
    expect(on).toBe(false)
  })

  test('a recipient without a key switches encryption off', () => {
    let value = true
    render(
      <EncryptToggle
        keys={keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })}
        on
        onChange={(on) => (value = on)}
      />,
    )
    expect(value).toBe(false)
  })

  test('a reply to encrypted mail stays encrypted when a recipient lacks a key; the lock looks it up', () => {
    // Switching it off by itself would send the reply plain (and skip the
    // core's rule that replies to encrypted mail stay encrypted).
    let value = true
    const { getByRole } = render(
      <EncryptToggle
        keys={keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })}
        on
        sensitive="reply"
        onChange={(on) => (value = on)}
      />,
    )
    expect(value).toBe(true)
    const sw = getByRole('switch') as HTMLButtonElement
    expect(sw.getAttribute('aria-checked')).toBe('true')
    expect(sw.title).toContain('bob@example.net')
    act(() => fireEvent.click(sw))
    expect(value).toBe(true)
    expect(crypto$.lookup.get()).toEqual({ addrs: ['bob@example.net'] })
    expect(ui$.confirm.get()).toBeFalsy()
  })

  test('once the lookup found nothing, the next click asks to turn a reply off', async () => {
    let value = true
    const missing = keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })
    const { getByRole } = render(<EncryptToggle keys={missing} on sensitive="reply" onChange={(on) => (value = on)} />)
    act(() => fireEvent.click(getByRole('switch'))) // the lookup
    expect(crypto$.lookup.get()).toEqual({ addrs: ['bob@example.net'] })
    expect(getByRole('switch').title).toBe("Can't encrypt to bob@example.net: remove them or turn off encryption.")
    act(() => fireEvent.click(getByRole('switch'))) // now the deliberate "off"
    await waitFor(() => expect(ui$.confirm.get()?.title).toBeTruthy())
    expect(value).toBe(true) // not until confirmed
  })

  test("a body holding an unfolded decrypted quote can't be switched to plain", () => {
    let value = true
    const { getByRole } = render(
      <EncryptToggle keys={keys()} on sensitive="reply" bodyIsDecrypted onChange={(on) => (value = on)} />,
    )
    const sw = getByRole('switch')
    expect(sw.getAttribute('title')).toBe('This message contains decrypted text, so it can only be sent encrypted.')
    act(() => fireEvent.click(sw))
    expect(value).toBe(true)
    expect(ui$.confirm.get()).toBeFalsy()
  })

  // Decrypted text in the body (an edited-as-new copy, a reopened draft)
  // without a sensitive source: a recipient lacking a key must not switch it
  // off; the lock offers the lookup instead.
  test('a body holding decrypted text stays encrypted when a recipient lacks a key', () => {
    let value = true
    const missing = keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })
    const { getByRole } = render(<EncryptToggle keys={missing} on bodyIsDecrypted onChange={(on) => (value = on)} />)
    expect(value).toBe(true)
    act(() => fireEvent.click(getByRole('switch')))
    expect(crypto$.lookup.get()).toEqual({ addrs: ['bob@example.net'] })
    expect(value).toBe(true)
  })

  test('the lock says when the quick reply leaves the original unquoted', () => {
    const { getByRole } = render(<EncryptToggle keys={keys()} on sensitive="reply" quoteWithheld onChange={() => {}} />)
    expect(getByRole('switch').getAttribute('title')).toStartWith("Encrypted and signed. The original isn't quoted")
    cleanup()
    const plain = render(<EncryptToggle keys={keys()} on sensitive="reply" onChange={() => {}} />)
    expect(plain.getByRole('switch').getAttribute('title')).toBe('Encrypted and signed')
  })

  test("a recipient's key that changed since the last mail turns the lock amber and says so", () => {
    const changed = [{ addr: 'bob@example.net', previous: 'AAAA', current: FPR }]
    const { getByRole } = render(<EncryptToggle keys={keys({ changed })} on onChange={() => {}} />)
    const lock = getByRole('switch')
    expect(lock.className).toContain('text-amber-600')
    expect(lock.getAttribute('title')).toContain("bob@example.net's key changed")
    cleanup()
    // A recipient without a key matters more: that message stays.
    const blocked = keys({ changed, missing: [{ addr: 'carl@example.net', reason: 'none' }] })
    const view = render(<EncryptToggle keys={blocked} on sensitive="forward" onChange={() => {}} />)
    expect(view.getByRole('switch').getAttribute('title')).toContain("Can't encrypt to carl@example.net")
  })

  test('a forward of encrypted mail stays encrypted and keeps Send blocked', () => {
    let value = true
    const missing = keys({ missing: [{ addr: 'bob@example.net', reason: 'none' }] })
    render(<EncryptToggle keys={missing} on sensitive="forward" onChange={(on) => (value = on)} />)
    expect(value).toBe(true)
    expect(encryptionBlocksSend(true, missing)).toBe(true)
  })

  test('with keys for everyone, encryption stays on', () => {
    let value = true
    render(<EncryptToggle keys={keys()} on onChange={(on) => (value = on)} />)
    expect(value).toBe(true)
  })

  test('a forward of encrypted mail stays encrypted', () => {
    let value = true
    const { getByRole } = render(<EncryptToggle keys={keys()} on sensitive="forward" onChange={(on) => (value = on)} />)
    fireEvent.click(getByRole('switch'))
    expect(value).toBe(true)
  })

  test('Send is blocked while encrypting to someone without a key', () => {
    expect(encryptionBlocksSend(true, keys({ missing: [{ addr: 'x@y.z', reason: 'none' }] }))).toBe(true)
    expect(encryptionBlocksSend(true, keys())).toBe(false)
    expect(encryptionBlocksSend(false, keys({ missing: [{ addr: 'x@y.z', reason: 'none' }] }))).toBe(false)
    expect(encryptionBlocksSend(true, null)).toBe(true)
  })
})

test('recipientAddresses splits like the core', () => {
  expect(recipientAddresses('"Doe, Jane" <Jane@Example.com>, bob@x.org', '', 'Carol <c@d.e>')).toEqual([
    'jane@example.com',
    'bob@x.org',
    'c@d.e',
  ])
})

test('an encrypted original is quoted only into an encrypted reply', async () => {
  const { composedContent } = await import('../../states/compose')
  const quote = { html: '<blockquote>secret</blockquote>', text: '> secret' }
  const draft = { rich: false, html: '', text: 'Thanks!', quote, sensitiveSource: 'reply' as const }
  expect(composedContent({ ...draft, encrypt: true })).toContain('> secret')
  expect(composedContent({ ...draft, encrypt: false })).toBe('Thanks!')
  expect(composedContent({ ...draft, sensitiveSource: undefined, encrypt: false })).toContain('> secret')
})

describe('key changes before sending', () => {
  // Another file can leave a dialog open; this block reads `ui$.confirm`.
  beforeEach(() => settleConfirm(false))
  afterEach(() => settleConfirm(false))

  test('asks once before sending to a changed key, and not otherwise', async () => {
    let changed: { addr: string; previous: string; current: string }[] = []
    ;(window as any).go.main.App.Invoke = async (command: string) =>
      command === 'pgp.keys.forRecipients' ? { enabled: true, own_key_unusable: false, missing: [], changed } : {}
    expect(await confirmKeyChanges('acc', ['bob@example.net'])).toBe(true)
    expect(ui$.confirm.get()).toBeNull()

    changed = [{ addr: 'bob@example.net', previous: 'AAAA', current: FPR }]
    const asked = confirmKeyChanges('acc', ['bob@example.net'])
    await waitFor(() => expect(ui$.confirm.get()?.message).toContain("bob@example.net's key changed"))
    settleConfirm(false)
    expect(await asked).toBe(false)

    const again = confirmKeyChanges('acc', ['bob@example.net'])
    await waitFor(() => expect(ui$.confirm.get()).toBeTruthy())
    settleConfirm(true)
    expect(await again).toBe(true)

    // A failed check doesn't hold the send back (the core still refuses missing keys).
    ;(window as any).go.main.App.Invoke = async () => {
      throw new Error('sidecar down')
    }
    expect(await confirmKeyChanges('acc', ['bob@example.net'])).toBe(true)
  })

  test('the note names the changed address and opens Verify for the new key', () => {
    const { getByText } = render(
      <KeyChangeNote accountId="acc" changed={[{ addr: 'bob@example.net', previous: 'AAAA', current: FPR }]} />,
    )
    expect(getByText(/bob@example.net's key changed/)).toBeTruthy()
    fireEvent.click(getByText('Verify…'))
    expect(crypto$.verify.get()).toEqual({
      addr: 'bob@example.net',
      name: 'bob@example.net',
      fingerprint: FPR,
      accountId: 'acc',
    })
  })
})

describe('KeyLookupDialog', () => {
  test('looks up only when opened, shows the source, and adds the picked key', async () => {
    const found = {
      source: { kind: 'hkp', url: 'https://keyserver.ubuntu.com', enabled: true },
      verified: false,
      info: { fingerprint: FPR, user_ids: ['Bob <bob@example.net>'], emails: ['bob@example.net'] },
      data_b64: 'AAAA',
    }
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      if (command === 'pgp.lookup')
        return { addr: payload.addr, found: [found], searched: ['WKD', 'keyserver.ubuntu.com'], failed: [] }
      if (command === 'pgp.lookup.add') return { key: found.info }
      return {}
    }
    const { findByText, getByText } = render(<KeyLookupDialog />)
    expect(calls.some((c) => c.command === 'pgp.lookup')).toBe(false)
    act(() => crypto$.lookup.set({ addrs: ['bob@example.net'] }))
    await findByText('Bob <bob@example.net>')
    expect(getByText(/doesn't check addresses/)).toBeTruthy()
    await act(async () => fireEvent.click(getByText('Use this key')))
    expect(calls.find((c) => c.command === 'pgp.lookup.add')?.payload).toEqual({
      addr: 'bob@example.net',
      data_b64: 'AAAA',
      source: 'hkp',
    })
  })

  test('says where it looked when nothing is found', async () => {
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) =>
      command === 'pgp.lookup'
        ? { addr: payload.addr, found: [], searched: ['WKD', 'keys.openpgp.org'], failed: [] }
        : {}
    const { findByText } = render(<KeyLookupDialog />)
    act(() => crypto$.lookup.set({ addrs: ['nobody@example.org'] }))
    expect(await findByText('No key found (WKD, keys.openpgp.org).')).toBeTruthy()
  })
})

describe('EnableEncryptionDialog', () => {
  test('a backup file asks for its code, then imports with it', async () => {
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      if (command === 'system.pickFiles') return { files: [{ name: 'backup.sec.pgp', data: 'data:;base64,QUJD' }] }
      if (command === 'pgp.account.importKey') {
        if (!payload.backup_code) return { result: { status: 'needs_backup_code', wrong: false } }
        if (payload.backup_code === 'nope') return { result: { status: 'needs_backup_code', wrong: true } }
        return {
          result: {
            status: 'imported',
            key: { fingerprint: FPR, user_ids: ['<me@example.org>'], emails: ['me@example.org'] },
            address_matches: true,
          },
        }
      }
      return {}
    }
    const { findByLabelText, findByText, getByText } = render(<EnableEncryptionDialog />)
    act(() => crypto$.enableFor.set({ accountId: 'acc', address: 'me@example.org' }))
    await act(async () => fireEvent.click(getByText('Use my existing key…')))
    const input = await findByLabelText('Backup code')
    fireEvent.change(input, { target: { value: 'nope' } })
    await act(async () => fireEvent.click(getByText('Use key')))
    expect(await findByText("That code didn't open the backup.")).toBeTruthy()
    fireEvent.change(input, { target: { value: '1234-5678' } })
    await act(async () => fireEvent.click(getByText('Use key')))
    await waitFor(() => expect(crypto$.enableFor.get()).toBe(null))
    const imports = calls.filter((c) => c.command === 'pgp.account.importKey')
    expect(imports.map((c) => c.payload.backup_code)).toEqual(['', 'nope', '1234-5678'])
    expect(imports.every((c) => c.payload.data_b64 === 'QUJD')).toBe(true)
  })
})

describe('SignToggle', () => {
  test('switches signing for this message, and is fixed on when encrypting', () => {
    const changes: boolean[] = []
    const view = render(<SignToggle on={false} encrypted={false} onChange={(on) => changes.push(on)} />)
    const off = view.getByRole('switch')
    expect(off.getAttribute('aria-checked')).toBe('false')
    fireEvent.click(off)
    expect(changes).toEqual([true])

    view.rerender(<SignToggle on={false} encrypted onChange={(on) => changes.push(on)} />)
    const locked = view.getByRole('switch') as HTMLButtonElement
    expect(locked.getAttribute('aria-checked')).toBe('true')
    expect(locked.disabled).toBe(true)
    expect(locked.title).toBe('Encrypted mail is always signed with your key.')
  })
})
