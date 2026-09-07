// Pure helpers for email address-list strings ("Name <addr>, addr2").

/** Split a "Name <addr>, addr2" list into individual entries, trimming empties.
 *
 * Commas inside a quoted display name ("Doe, Jane" <jane@x.com>) or inside
 * angle brackets do not separate entries: splitting on those produces fragments
 * that match no address, which then survive our own-address filtering and go
 * out as malformed recipients. Only the double quote opens a quoted string
 * (RFC 5322): an apostrophe is an ordinary character in a name like O'Connor,
 * and treating it as a delimiter swallows every recipient after it. Mirrors the
 * mobile splitter. Display parsers can opt into semicolon separators too. */
export function splitAddressList(raw: string | undefined | null, allowSemicolon = false): string[] {
  if (!raw) return []
  const entries: string[] = []
  let quoted = false
  let angleDepth = 0
  let start = 0
  const push = (end: number) => {
    const entry = raw.slice(start, end).trim()
    if (entry) entries.push(entry)
  }
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i]
    if (quoted) {
      if (ch === '\\') i += 1
      else if (ch === '"') quoted = false
    } else if (ch === '"') {
      quoted = true
    } else if (ch === '<') {
      angleDepth += 1
    } else if (ch === '>' && angleDepth > 0) {
      angleDepth -= 1
    } else if ((ch === ',' || (allowSemicolon && ch === ';')) && angleDepth === 0) {
      push(i)
      start = i + 1
    }
  }
  push(raw.length)
  return entries
}

/** Bare-address ("addr") form of a "Name <addr>" or "addr" entry, lowercased. */
export function bareAddr(entry: string): string {
  const match = entry.match(/<([^>]+)>/)
  return (match ? match[1] : entry).trim().toLowerCase()
}
