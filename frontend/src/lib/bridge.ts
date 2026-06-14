// The Wails IPC bridge: every backend call goes through here. In the packaged
// app (and `wails dev`) the runtime injects `window.go.main.App`; if it's absent
// there's no backend to talk to, so we fail loudly rather than silently no-op.
export async function invoke<T>(command: string, payload: unknown = {}): Promise<T> {
  const wailsApp = (window as any).go?.main?.App
  if (!wailsApp?.Invoke) {
    throw new Error(`Meron backend unavailable (no Wails bindings) for command "${command}"`)
  }
  return wailsApp.Invoke(command, payload)
}
