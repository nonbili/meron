// Wails can reject backend calls with a plain string, not an Error, so callers
// should normalize unknown failures before showing them in the UI.
export function errorMessage(err: unknown, fallback: string): string {
  if (err instanceof Error && err.message) return err.message
  if (typeof err === 'string' && err.trim()) return err
  if (
    err &&
    typeof err === 'object' &&
    'message' in err &&
    typeof (err as { message?: unknown }).message === 'string'
  ) {
    return (err as { message: string }).message
  }
  return fallback
}

export function contextualErrorMessage(err: unknown, fallback: string): string {
  const detail = errorMessage(err, fallback)
  return detail === fallback ? fallback : `${fallback}: ${detail}`
}
