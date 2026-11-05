import { expect, it, mock } from 'bun:test'

const calls: unknown[][] = []
let listener: (event: { data: unknown }) => void
let unsubscribed = false
mock.module('@wailsio/runtime', () => ({
  Call: {
    ByName: (...args: unknown[]) => {
      calls.push(args)
      return Promise.resolve({ ok: true })
    },
  },
  Events: {
    On: (_name: string, callback: typeof listener) => {
      listener = callback
      return () => {
        unsubscribed = true
      }
    },
  },
  Browser: { OpenURL: () => {} },
  Clipboard: { SetText: () => {} },
}))

const { installDesktopRuntime } = await import('./wails')

it('calls the v3 service by its generated binding name', async () => {
  installDesktopRuntime()
  const result = await (window as any).go.main.App.Invoke('core.status', {})
  expect(result).toEqual({ ok: true })
  expect(calls).toEqual([['main.desktopService.Invoke', 'core.status', {}]])
})

it('unwraps event data and preserves the unsubscribe function', () => {
  installDesktopRuntime()
  let detail: unknown
  const off = (window as any).runtime.EventsOn('mail.synced', (data: unknown) => {
    detail = data
  })
  listener({ data: { account: 'test' } })
  expect(detail).toEqual({ account: 'test' })
  off()
  expect(unsubscribed).toBe(true)
})
