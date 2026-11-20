import { useValue } from '@legendapp/state/react'
import { accounts$ } from '../../states/accounts'
import type { CalendarAccount } from '../../states/caldav'

export function useCalendarAccountIdentity(account?: CalendarAccount) {
  const mailboxes = useValue(accounts$)
  const mailbox = account?.google
    ? mailboxes.find((mailbox) => mailbox.email.trim().toLowerCase() === account.username.trim().toLowerCase())
    : undefined
  const savedName = account?.display_name
  // Older Google connections all stored the provider name as their default.
  const name =
    account?.google && (!savedName || savedName === 'Google Calendar')
      ? mailbox?.display_name.trim() || account.username
      : savedName || account?.server_url || ''
  return { name, avatarUrl: mailbox?.avatar_url }
}
