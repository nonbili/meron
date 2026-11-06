import { t } from '../lib/i18n'
import { showToast } from './ui'

/**
 * Addresses a send didn't reach. Encrypted mail goes out as one copy per Bcc,
 * so the server can refuse one after the others went out; the core reports
 * that instead of failing a send a retry would duplicate.
 */
export function undeliveredOf(result: unknown): string[] {
  const list = (result as { undelivered?: unknown } | null | undefined)?.undelivered
  return Array.isArray(list) ? list.filter((addr): addr is string => typeof addr === 'string') : []
}

/** Says who didn't get the mail. False when everyone did. */
export function warnUndelivered(addresses: string[]): boolean {
  if (addresses.length === 0) return false
  showToast(t('compose.toast.notDeliveredTo', { addresses: addresses.join(', ') }), 'error')
  return true
}
