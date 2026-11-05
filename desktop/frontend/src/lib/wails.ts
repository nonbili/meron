import { Browser, Call, Clipboard, Events } from '@wailsio/runtime'

// Keep the app's event payload contract independent of Wails' event envelope.
// Installed before React so existing subscriptions can share one adapter.
export function installDesktopRuntime() {
  ;(window as any).go = {
    main: {
      App: {
        Invoke: (command: string, payload: unknown) => Call.ByName('main.desktopService.Invoke', command, payload),
      },
    },
  }
  ;(window as any).runtime = {
    EventsOn: (name: string, callback: (data: unknown) => void) => Events.On(name, (event) => callback(event.data)),
    BrowserOpenURL: Browser.OpenURL,
    ClipboardSetText: Clipboard.SetText,
  }
}
