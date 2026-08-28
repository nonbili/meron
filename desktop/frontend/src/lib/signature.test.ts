import { describe, expect, it } from 'bun:test'
import type { Account } from '../types'
import {
  accountSignaturePayload,
  bodyWithSignature,
  bodyWithSwappedSignature,
  isBlankSignature,
  resolveSignature,
} from './signature'

const account = (overrides: Partial<Account> = {}): Account => ({
  id: 'acc',
  email: 'me@example.com',
  display_name: 'Me',
  provider: 'custom',
  auth_type: 'password',
  imap_host: 'imap.example.com',
  imap_port: 993,
  smtp_host: 'smtp.example.com',
  smtp_port: 465,
  tls: true,
  ...overrides,
})

describe('isBlankSignature', () => {
  it('treats empty markup as blank', () => {
    expect(isBlankSignature('')).toBe(true)
    expect(isBlankSignature('<p></p>')).toBe(true)
    expect(isBlankSignature('<p>&nbsp;</p>')).toBe(true)
  })

  it('keeps text and images', () => {
    expect(isBlankSignature('<p>Ping</p>')).toBe(false)
    expect(isBlankSignature('<p><img src="/media/logo.png"></p>')).toBe(false)
  })
})

describe('resolveSignature', () => {
  it('falls back to the app-wide signature when the account has no override', () => {
    expect(resolveSignature(account(), '<p>App</p>')).toBe('<p>App</p>')
  })

  it('honours the account override', () => {
    expect(resolveSignature(account({ signature: { mode: 'custom', html: '<p>Mine</p>' } }), '<p>App</p>')).toBe(
      '<p>Mine</p>',
    )
    expect(resolveSignature(account({ signature: { mode: 'none', html: '<p>Mine</p>' } }), '<p>App</p>')).toBe('')
    expect(resolveSignature(account({ signature: { mode: 'global', html: '<p>Mine</p>' } }), '<p>App</p>')).toBe(
      '<p>App</p>',
    )
  })

  it('resolves a blank signature to nothing', () => {
    expect(resolveSignature(account(), '<p></p>')).toBe('')
    expect(resolveSignature(undefined, '')).toBe('')
  })
})

describe('bodyWithSignature', () => {
  const rich = { rich: true, html: '', text: '' }
  const plain = { rich: false, html: '', text: '' }

  const sig = (html: string, text: string) => ({ html, text })

  it('leaves the body alone without a signature', () => {
    expect(bodyWithSignature(rich, sig('', ''))).toEqual(rich)
  })

  it('appends to a blank rich body with a line to type on', () => {
    expect(bodyWithSignature(rich, sig('<p>Ping</p>', 'Ping')).html).toBe('<p></p><p>Ping</p>')
  })

  it('puts the signature above a forwarded quote', () => {
    const quoted = { rich: true, html: '<blockquote>old</blockquote>', text: '' }
    expect(bodyWithSignature(quoted, sig('<p>Ping</p>', 'Ping'), 'aboveQuote').html).toBe(
      '<p></p><p>Ping</p><blockquote>old</blockquote>',
    )
  })

  it('keeps carried-over text above the signature', () => {
    const typed = { rich: true, html: '<p>typed</p>', text: '' }
    expect(bodyWithSignature(typed, sig('<p>Ping</p>', 'Ping')).html).toBe('<p>typed</p><p></p><p>Ping</p>')
  })

  it('uses the plaintext form for a plaintext draft', () => {
    expect(bodyWithSignature(plain, sig('<p>Ping</p><p>Pong</p>', 'Ping\n\nPong')).text).toBe('\n\nPing\n\nPong')
    expect(bodyWithSignature({ ...plain, text: 'typed' }, sig('<p>Ping</p>', 'Ping')).text).toBe('typed\n\nPing')
    expect(bodyWithSignature({ ...plain, text: '> quoted' }, sig('<p>Ping</p>', 'Ping'), 'aboveQuote').text).toBe(
      '\n\nPing\n\n> quoted',
    )
  })
})

describe('bodyWithSwappedSignature', () => {
  const sig = (html: string, text: string) => ({ html, text })
  const mine = sig('<p>Mine</p>', 'Mine')
  const theirs = sig('<p>Theirs</p>', 'Theirs')
  const none = sig('', '')
  const below = (s: { html: string; text: string }) => ({ ...s, placement: 'belowText' as const })
  const above = (s: { html: string; text: string }) => ({ ...s, placement: 'aboveQuote' as const })

  it('replaces the signature the draft was seeded with', () => {
    const body = { rich: true, html: '<p>typed</p><p></p><p>Mine</p>', text: '' }
    const out = bodyWithSwappedSignature(body, below(mine), theirs)

    expect(out.body.html).toBe('<p>typed</p><p></p><p>Theirs</p>')
    expect(out.tracking).toEqual(below(theirs))
  })

  it('adds one to a draft the app left without a signature', () => {
    const body = { rich: true, html: '<p>typed</p>', text: '' }
    const out = bodyWithSwappedSignature(body, null, theirs)

    expect(out.body.html).toBe('<p>typed</p><p></p><p>Theirs</p>')
    expect(out.tracking).toEqual(below(theirs))
  })

  it('never touches a body it did not compose', () => {
    // A reopened draft already ends in whatever signature it was written with:
    // appending here is how a message ends up with two.
    const body = { rich: true, html: '<p>hello</p><p>Mine</p>', text: '' }
    const out = bodyWithSwappedSignature(body, undefined, theirs)

    expect(out.body).toEqual(body)
    expect(out.tracking).toBeUndefined()
  })

  it('stops tracking a signature the user has edited, and leaves it alone', () => {
    const body = { rich: true, html: '<p>typed</p><p></p><p>Mine, edited</p>', text: '' }
    const out = bodyWithSwappedSignature(body, below(mine), theirs)

    expect(out.body).toEqual(body)
    expect(out.tracking).toBeUndefined()
  })

  it('removes the signature and the blank line it came with', () => {
    const body = { rich: true, html: '<p>typed</p><p></p><p>Mine</p>', text: '' }
    const out = bodyWithSwappedSignature(body, below(mine), none)

    expect(out.body.html).toBe('<p>typed</p>')
    // Not simply "none": the placement survives, so the next account's
    // signature goes back where this one was.
    expect(out.tracking).toEqual({ html: '', text: '', placement: 'belowText' })
  })

  it('does not accumulate blank paragraphs across a round trip', () => {
    const start = { rich: true, html: '<p>typed</p>', text: '' }
    const added = bodyWithSwappedSignature(start, null, mine)
    const removed = bodyWithSwappedSignature(added.body, added.tracking, none)
    const readded = bodyWithSwappedSignature(removed.body, removed.tracking, theirs)

    expect(added.body.html).toBe('<p>typed</p><p></p><p>Mine</p>')
    expect(removed.body.html).toBe('<p>typed</p>')
    expect(readded.body.html).toBe('<p>typed</p><p></p><p>Theirs</p>')
  })

  it('appends without reformatting whitespace the user left in the body', () => {
    const body = { rich: false, html: '', text: '  indented start\n\nand a trailing space \n' }
    const out = bodyWithSwappedSignature(body, null, mine)

    expect(out.body.text).toBe('  indented start\n\nand a trailing space \n\nMine')
  })

  it('swaps the plaintext form for a plaintext draft', () => {
    const body = { rich: false, html: '', text: 'typed\n\nMine' }
    expect(bodyWithSwappedSignature(body, below(mine), theirs).body.text).toBe('typed\n\nTheirs')
    expect(bodyWithSwappedSignature(body, below(mine), none).body.text).toBe('typed')
  })

  it('leaves a plaintext signature that has been written into alone', () => {
    const body = { rich: false, html: '', text: 'typed\n\nMine, but edited' }
    expect(bodyWithSwappedSignature(body, below(mine), theirs).body).toEqual(body)
  })

  it('remembers where a signature belongs even when the account had none', () => {
    // A forward opened under an account with no signature: the quote is the
    // whole body, and the mark remembers that a signature goes above it.
    const forwarded = { rich: false, html: '', text: '\n\n> Forwarded message' }
    const out = bodyWithSwappedSignature(forwarded, { html: '', text: '', placement: 'aboveQuote' }, mine)

    expect(out.body.text).toBe('\n\nMine\n\n> Forwarded message')
    expect(out.tracking).toEqual(above(mine))
  })

  it('refuses to rewrite anything when the body holds a second identical block', () => {
    // The user pasted their signature into the message as well. Which copy is
    // ours is a guess, and guessing wrong rewrites their words — so neither is
    // touched, and the draft stops being managed.
    const body = { rich: false, html: '', text: 'Mine\n\nis how I sign off\n\nMine\n\nPS. one more thing' }
    const out = bodyWithSwappedSignature(body, below(mine), theirs)

    expect(out.body).toEqual(body)
    expect(out.tracking).toBeUndefined()
  })

  it('still swaps an ambiguous body when its own copy is untouched at the edge', () => {
    // Same duplication, but ours is still the last block, exactly where it was
    // inserted: that is unambiguous enough to swap.
    const body = { rich: false, html: '', text: 'Mine\n\nis how I sign off\n\nMine' }
    const out = bodyWithSwappedSignature(body, below(mine), theirs)

    expect(out.body.text).toBe('Mine\n\nis how I sign off\n\nTheirs')
  })

  it('does not reach inside the user’s own blocks to swap nested markup', () => {
    // The signature's markup also appears inside a quote the user is replying
    // to. That copy is their content, not ours, and must survive untouched.
    const body = {
      rich: true,
      html: '<blockquote><p>Mine</p></blockquote><p></p><p>Mine</p>',
      text: '',
    }
    const out = bodyWithSwappedSignature(body, below(mine), theirs)

    expect(out.body.html).toBe('<blockquote><p>Mine</p></blockquote><p></p><p>Theirs</p>')
  })

  it('leaves a signature alone when only a nested copy of it remains', () => {
    // The real one was deleted; what is left looks identical but belongs to the
    // quote around it, so nothing is rewritten.
    const body = { rich: true, html: '<blockquote><p>Mine</p></blockquote>', text: '' }
    const out = bodyWithSwappedSignature(body, below(mine), theirs)

    expect(out.body).toEqual(body)
    expect(out.tracking).toBeUndefined()
  })

  it('does not rewrite a container-wrapped rich fragment', () => {
    const wrapped = { html: '<div><p>Mine</p></div>', text: 'Mine' }
    const body = { rich: true, html: '<div><p>Mine</p></div>', text: '' }
    const out = bodyWithSwappedSignature(body, below(wrapped), theirs)

    expect(out.body).toEqual(body)
    expect(out.tracking).toBeUndefined()
  })

  it('swaps a multi-block signature as a unit', () => {
    const twoBlocks = { html: '<p>Mine</p><p>Team</p>', text: 'Mine\nTeam' }
    const body = { rich: true, html: '<p>typed</p><p></p><p>Mine</p><p>Team</p>', text: '' }
    const out = bodyWithSwappedSignature(body, { ...twoBlocks, placement: 'belowText' }, theirs)

    expect(out.body.html).toBe('<p>typed</p><p></p><p>Theirs</p>')
  })

  it('swaps the copy it inserted, not an identical one in the quote', () => {
    // Forward of a message that ends in the same signature: ours is the one
    // above the quote, and the quoted copy must survive untouched.
    const body = { rich: false, html: '', text: '\n\nMine\n\n> Forwarded\n> Mine' }
    const out = bodyWithSwappedSignature(body, above(mine), theirs)

    expect(out.body.text).toBe('\n\nTheirs\n\n> Forwarded\n> Mine')
  })

  it('swaps the last copy when the signature sits below the text', () => {
    const body = { rich: false, html: '', text: 'Mine\n\nis what I always write\n\nMine' }
    const out = bodyWithSwappedSignature(body, below(mine), theirs)

    expect(out.body.text).toBe('Mine\n\nis what I always write\n\nTheirs')
  })

  it('closes only the seam it cut, leaving other blank lines alone', () => {
    const body = { rich: false, html: '', text: 'one\n\n\n\ntwo\n\nMine\n\n> quoted\n\n\n> lines' }
    const out = bodyWithSwappedSignature(body, below(mine), none)

    expect(out.body.text).toBe('one\n\n\n\ntwo\n\n> quoted\n\n\n> lines')
  })

  it('keeps the blank line above a quote when the signature goes', () => {
    const body = { rich: false, html: '', text: '\n\nMine\n\n> quoted' }
    expect(bodyWithSwappedSignature(body, above(mine), none).body.text).toBe('\n\n> quoted')
  })
})

describe('accountSignaturePayload', () => {
  it('clears the override only when nothing was written', () => {
    expect(accountSignaturePayload('global', '')).toBeNull()
    expect(accountSignaturePayload('global', '<p>Mine</p>')).toEqual({ mode: 'global', html: '<p>Mine</p>' })
  })

  it('keeps the text when the account opts out, so switching back restores it', () => {
    expect(accountSignaturePayload('none', '<p>Mine</p>')).toEqual({ mode: 'none', html: '<p>Mine</p>' })
    expect(accountSignaturePayload('custom', '<p>Mine</p>')).toEqual({ mode: 'custom', html: '<p>Mine</p>' })
  })
})
