import { expect, it } from 'bun:test'
import { usesNativeScrollbars } from './nativeScrollbars'

const LINUX = 'Mozilla/5.0 (X11; Linux aarch64) AppleWebKit/605.1.15 (KHTML, like Gecko)'
// WKWebView reports an Intel Mac on Apple silicon too.
const MACOS = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)'
const WINDOWS =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0'

it('leaves scrollbars to WebKitGTK on Linux', () => {
  expect(usesNativeScrollbars(LINUX)).toBe(true)
})

// macOS overlay scrollbars follow "Show scroll bars: Automatically", which the
// styled ::-webkit-scrollbar ones ignore, staying visible while idle.
it('leaves scrollbars to WKWebView on macOS', () => {
  expect(usesNativeScrollbars(MACOS)).toBe(true)
})

it('keeps the styled scrollbars on Windows', () => {
  expect(usesNativeScrollbars(WINDOWS)).toBe(false)
})
