// The Wails IPC bridge: every backend call goes through here. In the packaged
// app (and `wails3 dev`) lib/wails installs the v3 command adapter; if it's absent
// there's no backend to talk to, so we fail loudly rather than silently no-op.
export async function invoke<T>(command: string, payload: unknown = {}): Promise<T> {
  const wailsApp = (window as any).go?.main?.App
  if (!wailsApp?.Invoke) {
    throw new Error(`Meron backend unavailable (no Wails bindings) for command "${command}"`)
  }
  return wailsApp.Invoke(command, payload)
}

/** Whether `error` is the Go bridge giving up on a slow core (`sidecar.go`),
 *  as opposed to an answer from the core. */
export function isSidecarTimeout(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error)
  return /^sidecar \S+ timeout after /.test(message)
}

/**
 * `invoke` for a read the app cannot start without, retried while the core is
 * too busy to answer in time — at startup it may still be opening or migrating
 * the store. Only timeouts are retried; the core's own errors are returned
 * as-is. Callers must only pass requests that are safe to send twice, since a
 * timed-out request may still be answered (and discarded) later.
 */
export async function invokeRetryingTimeout<T>(command: string, payload: unknown = {}, attempts = 6): Promise<T> {
  for (let attempt = 1; ; attempt++) {
    try {
      return await invoke<T>(command, payload)
    } catch (error) {
      if (attempt >= attempts || !isSidecarTimeout(error)) throw error
      console.warn(`${command} timed out (attempt ${attempt}/${attempts}); retrying`)
    }
  }
}
