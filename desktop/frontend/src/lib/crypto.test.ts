import { describe, expect, test } from 'bun:test'
import {
  decryptFailed,
  fingerprintsMatch,
  formatFingerprint,
  indicatorFor,
  mayQuoteInto,
  mayQuoteOriginal,
  shortFingerprint,
  stripFor,
  type MessageCrypto,
} from './crypto'

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

describe('indicatorFor', () => {
  test('ordinary mail shows nothing', () => {
    expect(indicatorFor(undefined)).toBeNull()
    expect(indicatorFor({ ...base, offer: true })).toBeNull()
  })

  test('the icon says encrypted or signed, the tone says trust', () => {
    expect(indicatorFor({ ...base, encrypted: true, signed: true, trust: 'verified' })).toEqual({
      kind: 'lock',
      tone: 'verified',
    })
    expect(indicatorFor({ ...base, encrypted: true, signed: true, trust: 'known' })).toEqual({
      kind: 'lock',
      tone: 'known',
    })
    expect(indicatorFor({ ...base, encrypted: true })).toEqual({ kind: 'lock', tone: 'unconfirmed' })
    expect(indicatorFor({ ...base, signed: true, trust: 'verified' })).toEqual({ kind: 'signature', tone: 'verified' })
    expect(indicatorFor({ ...base, signed: true, trust: 'known' })).toEqual({ kind: 'signature', tone: 'known' })
    expect(indicatorFor({ ...base, signed: true })).toEqual({ kind: 'signature', tone: 'unconfirmed' })
  })

  test('problems and a bypassed verified key are danger', () => {
    expect(indicatorFor({ ...base, signed: true, problem: 'invalid_signature' })?.tone).toBe('danger')
    expect(indicatorFor({ ...base, encrypted: true, problem: 'integrity' })?.tone).toBe('danger')
    expect(
      indicatorFor({ ...base, signed: true, trust: 'known', key_changed: true, verified_conflict: true })?.tone,
    ).toBe('danger')
  })
})

describe('stripFor', () => {
  test('problems come first, then key changes, then the offer', () => {
    expect(stripFor({ ...base, problem: 'no_key', key_changed: true, offer: true })).toEqual({
      kind: 'problem',
      problem: 'no_key',
      danger: true,
    })
    expect(stripFor({ ...base, key_changed: true, offer: true })).toEqual({ kind: 'key_changed', danger: false })
    expect(stripFor({ ...base, key_changed: true, verified_conflict: true })).toEqual({
      kind: 'key_changed',
      danger: true,
    })
    expect(stripFor({ ...base, offer: true })).toEqual({ kind: 'offer' })
    expect(stripFor(base)).toBeNull()
  })

  test('inline encryption in HTML is a neutral note, not an alarm', () => {
    expect(stripFor({ ...base, problem: 'inline_in_html' })).toEqual({
      kind: 'problem',
      problem: 'inline_in_html',
      danger: false,
    })
  })
})

test('decryptFailed covers only decryption failures', () => {
  expect(decryptFailed({ ...base, encrypted: true, problem: 'integrity' })).toBe(true)
  expect(decryptFailed({ ...base, signed: true, problem: 'invalid_signature' })).toBe(false)
})

describe('fingerprints', () => {
  const fpr = '8E9BCE8647564E257A411B71969BC97D064D88D5'

  test('format in groups of four', () => {
    expect(formatFingerprint(fpr)).toBe('8E9B CE86 4756 4E25 7A41 1B71 969B C97D 064D 88D5')
    expect(shortFingerprint(fpr)).toBe('969B C97D 064D 88D5')
  })

  test('a pasted fingerprint matches regardless of spaces, case and colons', () => {
    expect(fingerprintsMatch('8e9b ce86 4756 4e25 7a41 1b71 969b c97d 064d 88d5', fpr)).toBe(true)
    expect(fingerprintsMatch('8E:9B:CE:86:47:56:4E:25:7A:41:1B:71:96:9B:C9:7D:06:4D:88:D5', fpr)).toBe(true)
    expect(fingerprintsMatch('8E9BCE8647564E257A411B71969BC97D064D88D6', fpr)).toBe(false)
  })

  test('a short or empty paste never matches', () => {
    expect(fingerprintsMatch('064D 88D5', fpr)).toBe(false)
    expect(fingerprintsMatch('', fpr)).toBe(false)
  })
})

test('a decrypted message is never quoted into an unencrypted reply', () => {
  const encrypted = { crypto: { ...base, encrypted: true } }
  expect(mayQuoteInto(encrypted, false)).toBe(false)
  expect(mayQuoteInto(encrypted, true)).toBe(true)
  expect(mayQuoteInto({ crypto: null }, false)).toBe(true)
  expect(mayQuoteInto(undefined, false)).toBe(true)
})

test('decrypted mail is quotable only when its sender is shown to have written it', () => {
  expect(mayQuoteOriginal(null)).toBe(true)
  // Plain mail with an unverifiable signature is ordinary mail: quoted as before.
  expect(mayQuoteOriginal({ ...base, signed: true })).toBe(true)
  expect(mayQuoteOriginal({ ...base, encrypted: true, signed: true, trust: 'known' })).toBe(true)
  expect(mayQuoteOriginal({ ...base, encrypted: true, signed: true, trust: 'verified' })).toBe(true)
  expect(mayQuoteOriginal({ ...base, encrypted: true })).toBe(false)
  expect(mayQuoteOriginal({ ...base, encrypted: true, signed: true, problem: 'signer_mismatch' })).toBe(false)
})

test('a signature by a revoked or expired key is danger, never verified', () => {
  expect(indicatorFor({ ...base, signed: true, trust: 'verified', problem: 'key_revoked' })?.tone).toBe('danger')
  expect(indicatorFor({ ...base, signed: true, problem: 'key_expired' })?.tone).toBe('danger')
  expect(stripFor({ ...base, signed: true, problem: 'key_revoked' })).toEqual({
    kind: 'problem',
    problem: 'key_revoked',
    danger: true,
  })
})
