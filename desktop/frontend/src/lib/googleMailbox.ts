import type { Account } from '../types'

/** Google mail may also be configured as generic IMAP with an app password. */
export function isGoogleMailbox(account: Account) {
  const domain = account.email.trim().toLowerCase().split('@').at(-1)
  const host = account.imap_host.trim().toLowerCase().replace(/\.$/, '')
  return (
    account.auth_type === 'gmail_oauth' ||
    account.provider === 'gmail' ||
    domain === 'gmail.com' ||
    domain === 'googlemail.com' ||
    host === 'imap.gmail.com' ||
    host === 'imap.googlemail.com'
  )
}
