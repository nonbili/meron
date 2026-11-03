import { expect, it } from 'bun:test'
import { splitAddressList } from './address'
import { formatContact } from './contacts'

it('quotes display names that would otherwise split the address list', () => {
  expect(formatContact({ name: 'Aki', addr: 'aki@example.com' })).toBe('Aki <aki@example.com>')
  expect(formatContact({ name: '', addr: 'aki@example.com' })).toBe('aki@example.com')
  const jane = formatContact({ name: 'Doe, Jane', addr: 'jane@example.com' })
  expect(jane).toBe('"Doe, Jane" <jane@example.com>')
  expect(splitAddressList(`${jane}, bob@example.com`)).toEqual([jane, 'bob@example.com'])
  expect(formatContact({ name: 'Say "hi"', addr: 'hi@example.com' })).toBe('"Say \\"hi\\"" <hi@example.com>')
})
