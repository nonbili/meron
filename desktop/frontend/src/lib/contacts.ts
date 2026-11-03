import { invoke } from './bridge'
import type { Contact } from '../types'

// Recipient autocomplete suggestions, drawn from the senders of cached messages
// by the sidecar. `accountId` scopes the lookup to one account (pass "" for a
// unified search across all accounts).
export async function suggestContacts(accountId: string, query: string, limit = 8): Promise<Contact[]> {
  try {
    const res = await invoke<{ contacts?: Contact[] }>('mail.suggestContacts', {
      account: accountId,
      query,
      limit,
    })
    return res.contacts ?? []
  } catch {
    return []
  }
}

// Render a contact as a recipient header entry: "Name <addr>" when a distinct
// display name exists, otherwise the bare address. A name holding an RFC 5322
// special (the comma in "Doe, Jane") is quoted, or the address list would split
// it into a bogus recipient.
export function formatContact(c: Contact): string {
  const name = c.name.trim()
  if (name && name.toLowerCase() !== c.addr.toLowerCase()) {
    const display = /[()<>[\]:;@\\,."]/.test(name) ? `"${name.replace(/["\\]/g, '\\$&')}"` : name
    return `${display} <${c.addr}>`
  }
  return c.addr
}
