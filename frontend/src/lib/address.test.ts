import { describe, expect, it } from 'bun:test'
import { bareAddr, splitAddressList } from './address'

describe('splitAddressList', () => {
  it('splits a comma-separated list and trims entries', () => {
    expect(splitAddressList('Alice <a@x.com>, b@y.com ,c@z.com')).toEqual(['Alice <a@x.com>', 'b@y.com', 'c@z.com'])
  })

  it('drops empty entries', () => {
    expect(splitAddressList('a@x.com,, ,b@y.com')).toEqual(['a@x.com', 'b@y.com'])
  })

  it('returns [] for empty, null, and undefined input', () => {
    expect(splitAddressList('')).toEqual([])
    expect(splitAddressList(null)).toEqual([])
    expect(splitAddressList(undefined)).toEqual([])
  })
})

describe('bareAddr', () => {
  it('extracts the address from a Name <addr> entry', () => {
    expect(bareAddr('Alice Example <Alice@X.com>')).toBe('alice@x.com')
  })

  it('lowercases and trims a bare address', () => {
    expect(bareAddr('  Bob@Y.COM ')).toBe('bob@y.com')
  })

  it('passes through entries without angle brackets', () => {
    expect(bareAddr('no-at-sign')).toBe('no-at-sign')
  })
})
