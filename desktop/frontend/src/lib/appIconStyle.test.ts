import { expect, test } from 'bun:test'
import {
  appIconHintKey,
  createAppIconUpdater,
  DEFAULT_APP_ICON_COLOUR,
  parseAppIconColour,
  type AppIcon,
} from './appIconStyle'

const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

test('anything that is not a colour falls back to the default', () => {
  expect(parseAppIconColour('#D0342C')).toBe('#d0342c')
  expect(parseAppIconColour('#fff')).toBe(DEFAULT_APP_ICON_COLOUR)
  expect(parseAppIconColour('neon')).toBe(DEFAULT_APP_ICON_COLOUR)
  expect(parseAppIconColour(undefined)).toBe(DEFAULT_APP_ICON_COLOUR)
})

test('the hint names what the platform can change', () => {
  expect(appIconHintKey('darwin')).toBe('settings.appearance.appIconHint')
  expect(appIconHintKey('linux')).toBe('settings.appearance.appIconHintLinux')
  expect(appIconHintKey('windows')).toBe('settings.appearance.appIconHintWindows')
})

test('a drag through the picker ends on the last colour', async () => {
  let resolve!: () => void
  const first = new Promise<void>((done) => {
    resolve = done
  })
  const sent: AppIcon[] = []
  const update = createAppIconUpdater(async (icon) => {
    sent.push(icon)
    if (sent.length === 1) await first
  })

  update({ custom: true, colour: '#111111' })
  update({ custom: true, colour: '#222222' })
  update({ custom: true, colour: '#333333' })
  resolve()
  await tick()
  await tick()

  expect(sent).toEqual([
    { custom: true, colour: '#111111' },
    { custom: true, colour: '#333333' },
  ])
})

test('the same choice is not sent twice', async () => {
  const sent: AppIcon[] = []
  const update = createAppIconUpdater(async (icon) => {
    sent.push(icon)
  })
  update({ custom: true, colour: '#111111' })
  await tick()
  update({ custom: true, colour: '#111111' })
  await tick()
  expect(sent).toHaveLength(1)
})
