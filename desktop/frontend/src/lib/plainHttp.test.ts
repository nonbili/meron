import { expect, it } from 'bun:test'
import { sendsPlainText } from './plainHttp'

it('flags plain http to other machines only', () => {
  expect(sendsPlainText('http://192.0.2.10:4080')).toBe(true)
  expect(sendsPlainText(' HTTP://dav.example.test/ ')).toBe(true)
  expect(sendsPlainText('https://dav.example.test')).toBe(false)
  expect(sendsPlainText('dav.example.test')).toBe(false)
  expect(sendsPlainText('http://127.0.0.1:4080')).toBe(false)
  expect(sendsPlainText('http://localhost:4080')).toBe(false)
  expect(sendsPlainText('http://[::1]:4080')).toBe(false)
})
