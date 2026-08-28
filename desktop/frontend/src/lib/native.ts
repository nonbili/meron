// Native shell helpers that route through the Wails runtime.

// Open a URL in the user's default browser. A bare `<a target="_blank">` does
// nothing inside the Wails webview (it tries to navigate the app frame), so we
// route through the runtime's BrowserOpenURL.
export function openExternal(url: string) {
  const open = (window as any).runtime?.BrowserOpenURL
  if (open) {
    open(url)
  } else {
    window.open(url, '_blank', 'noreferrer')
  }
}
