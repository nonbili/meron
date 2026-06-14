// Pure helpers for email address-list strings ("Name <addr>, addr2").

/** Split a "Name <addr>, addr2" list into individual entries, trimming empties. */
export function splitAddressList(raw: string | undefined | null): string[] {
  if (!raw) return []
  return raw
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
}

/** Bare-address ("addr") form of a "Name <addr>" or "addr" entry, lowercased. */
export function bareAddr(entry: string): string {
  const match = entry.match(/<([^>]+)>/)
  return (match ? match[1] : entry).trim().toLowerCase()
}
