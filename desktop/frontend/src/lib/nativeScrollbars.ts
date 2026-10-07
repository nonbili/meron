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
