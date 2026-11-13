import { expect, test } from 'bun:test'
import { createAppIconStyleUpdater, parseAppIconStyle, type AppIconStyle } from './appIconStyle'

const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

test('unknown styles fall back to the shipped icon', () => {
  expect(parseAppIconStyle('dark')).toBe('dark')
  expect(parseAppIconStyle('blue')).toBe('blue')
  expect(parseAppIconStyle('auto')).toBe('auto')
  expect(parseAppIconStyle('neon')).toBe('default')
  expect(parseAppIconStyle(undefined)).toBe('default')
})

test('quick style changes finish on the newest choice', async () => {
  let resolve!: () => void
  const first = new Promise<void>((done) => {
    resolve = done
  })
  const sent: AppIconStyle[] = []
  const update = createAppIconStyleUpdater(async (style) => {
    sent.push(style)
    if (sent.length === 1) await first
  })
  update('light')
  update('dark')
  update('auto')
  expect(sent).toEqual(['light'])
  resolve()
  await tick()
  expect(sent).toEqual(['light', 'auto'])
})
