/**
 * Whether the page should leave scrollbars to the webview (`html.native-scrollbars`).
 * WebKitGTK and WKWebView draw the desktop's overlay scrollbars (no gutter, fade
 * out when idle, following macOS's "Show scroll bars" setting) unless the page
 * styles them, so index.css leaves them alone on Linux and macOS. WebView2 on
 * Windows keeps the styled ones.
 */
export function usesNativeScrollbars(userAgent: string): boolean {
  return /linux|macintosh/i.test(userAgent)
}

/** Roughly how wide the webview's overlay scrollbar is under the pointer. */
const OVERLAY_SCROLLBAR_WIDTH = 16

/**
 * How far in from a scroll container's right edge a press lands on its vertical
 * scrollbar. A styled scrollbar takes layout space, which measures it; an
 * overlay one takes none, so a container that scrolls gets a nominal width.
 */
export function scrollbarHitWidth(
  container: Pick<HTMLElement, 'offsetWidth' | 'clientWidth' | 'scrollHeight' | 'clientHeight'>,
  nativeScrollbars: boolean,
): number {
  const gutter = container.offsetWidth - container.clientWidth
  if (gutter > 0) return gutter
  return nativeScrollbars && container.scrollHeight > container.clientHeight ? OVERLAY_SCROLLBAR_WIDTH : 0
}
