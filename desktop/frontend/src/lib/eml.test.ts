import { afterEach, expect, it } from 'bun:test'
import { openEmlFile } from './eml'

const previousGo = (window as any).go
afterEach(() => {
  ;(window as any).go = previousGo
})

it('shares one pending picker across entry points and resets after cancellation or failure', async () => {
  const calls: unknown[] = []
  let finish!: (result: unknown) => void
  let fail!: (reason: Error) => void
  ;(window as any).go = {
    main: {
      App: {
        Invoke: (command: string, payload: unknown) => {
          calls.push({ command, payload })
          return new Promise((resolve, reject) => {
            finish = resolve
            fail = reject
          })
        },
      },
    },
  }
  const first = openEmlFile('reader-1')
  expect(openEmlFile('reader-1')).toBe(first)
  expect(openEmlFile()).toBe(first)
  expect(calls).toEqual([{ command: 'eml.open', payload: { session: 'reader-1' } }])
  finish({ opened: false })
  await first

  const second = openEmlFile()
  expect(calls).toHaveLength(2)
  fail(new Error('Picker failed'))
  await expect(second).rejects.toThrow('Picker failed')

  const third = openEmlFile()
  expect(calls).toHaveLength(3)
  finish({ opened: true })
  await third
})
