import { expect, it } from 'bun:test'
import { normalizeRecipientInput, pasteRecipientInput } from './recipientInput'

it('normalizes Excel rows, columns, rectangles, and mixed separators', () => {
  for (const separator of [',', '; ', '\t', '\r\n', ' ', '  ', '\t\r\n']) {
    expect(normalizeRecipientInput(`a@x.com${separator}b@y.com`)).toBe('a@x.com, b@y.com')
  }
  expect(normalizeRecipientInput('a@x.com\tb@y.com\r\nc@z.com\td@z.com\r\n')).toBe(
    'a@x.com, b@y.com, c@z.com, d@z.com, ',
  )
})

it('preserves display names, escapes, unfinished input, and unrecognized cells', () => {
  for (const entry of ['Alice Smith <a@x.com>', '"Doe, Jane; Q" <a@x.com>', '"A \\" B" <a@x.com>', '"Doe,']) {
    expect(normalizeRecipientInput(entry)).toBe(entry)
  }
  expect(normalizeRecipientInput(String.raw`"A \" B" <a@x.com>; b@y.com`)).toBe(String.raw`"A \" B" <a@x.com>, b@y.com`)
  expect(normalizeRecipientInput('Email\r\na@x.com\tinvalid')).toBe('Email, a@x.com, invalid')
  expect(normalizeRecipientInput('a@x.com not-an-address')).toBe('a@x.com not-an-address')
  expect(normalizeRecipientInput('a@x.com ')).toBe('a@x.com, ')
})

it('recovers at spreadsheet cell boundaries after malformed names or brackets', () => {
  for (const malformed of ['Bob <b@y.com', '"Bob <b@y.com']) {
    expect(normalizeRecipientInput(`${malformed}\tc@z.com\r\nd@w.com\r\n`)).toBe(`${malformed}\tc@z.com\td@w.com\t`)
  }
})

it('maps the paste caret through separator changes caused by malformed suffixes', () => {
  const result = pasteRecipientInput('a@x.com\t"Bob <b@y.com\tc@z.com', 0, 0, 'd@d.com\te@e.com\t')
  expect(result.value).toBe('d@d.com\te@e.com\ta@x.com\t"Bob <b@y.com\tc@z.com')
  expect(result.caret).toBe('d@d.com\te@e.com\t'.length)
})
