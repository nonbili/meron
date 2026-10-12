import { expect, it } from 'bun:test'
import { offersMonochromeTrayIcon } from './monochromeTrayIcon'

const LINUX = 'Mozilla/5.0 (X11; Linux aarch64) AppleWebKit/605.1.15 (KHTML, like Gecko)'
const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)'
const WINDOWS = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Edg/141.0.0.0'

it('offers the monochrome tray icon where the bar draws one-colour icons', () => {
  expect(offersMonochromeTrayIcon(LINUX)).toBe(true)
  expect(offersMonochromeTrayIcon(MAC)).toBe(true)
  expect(offersMonochromeTrayIcon(WINDOWS)).toBe(false)
  expect(offersMonochromeTrayIcon('reduced user agent', 'MacIntel')).toBe(true)
  expect(offersMonochromeTrayIcon('reduced user agent', 'Linux x86_64')).toBe(true)
  expect(offersMonochromeTrayIcon('reduced user agent', 'Win32')).toBe(false)
})
