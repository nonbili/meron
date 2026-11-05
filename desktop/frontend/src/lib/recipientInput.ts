// Composer input is more permissive than an RFC header. Keep this scanner in
// sync with mobile's recipientSeparatorIndices and SMTP's parse_addrs.
function isBareEmail(value: string): boolean {
  return /^[^\s<>@,;"\\]+@[^\s<>@,;"\\]+$/.test(value)
}

export function recipientSeparatorIndices(value: string): number[] {
  const indices: number[] = []
  let quoted = false
  let depth = 0
  let start = 0
  for (let i = 0; i < value.length; i++) {
    const ch = value[i]
    // Spreadsheet cell boundaries recover even from an unfinished name/address.
    if (/[\t\r\n]/.test(ch)) {
      indices.push(i)
      start = i + 1
      quoted = false
      depth = 0
      continue
    }
    if (quoted) {
      if (ch === '\\') i++
      else if (ch === '"') quoted = false
      continue
    }
    if (ch === '"') quoted = true
    else if (ch === '<') depth++
    else if (ch === '>' && depth > 0) depth--
    else if (depth === 0) {
      const hard = ch === ',' || ch === ';'
      let space = false
      if (ch === ' ' && isBareEmail(value.slice(start, i).trim())) {
        const rest = value.slice(i + 1).trimStart()
        space = !rest || isBareEmail(rest.split(/[\s,;]/, 1)[0])
      }
      if (hard || space) {
        indices.push(i)
        start = i + 1
      }
    }
  }
  return indices
}

/** Normalize separators without dropping unrecognized text or the active tail. */
export function normalizeRecipientInput(value: string): string {
  const indices = recipientSeparatorIndices(value)
  if (!indices.length) return value
  const entries: string[] = []
  let start = 0
  for (const index of indices) {
    const entry = value.slice(start, index).trim()
    if (entry) entries.push(entry)
    start = index + 1
  }
  const tail = value.slice(start).trimStart()
  // Keep hard cell boundaries if a malformed entry would swallow a comma on
  // the next parse. Tabs survive a single-line input, unlike CR/LF.
  if (entries.some((entry) => !recipientSeparatorIndices(`${entry},`).includes(entry.length))) {
    return [...entries, tail].join('\t')
  }
  return entries.length ? `${entries.join(', ')}, ${tail}` : tail
}

export function pasteRecipientInput(
  value: string,
  start: number,
  end: number,
  text: string,
): { value: string; caret: number } {
  const prefix = normalizeRecipientInput(value.slice(0, start) + text)
  const suffix = value.slice(end)
  // Map through the final normalization, which may change separators before
  // the caret if a malformed entry occurs in the suffix.
  let marker = '\uE000'
  while ((prefix + suffix).includes(marker)) marker += '\uE000'
  const marked = normalizeRecipientInput(prefix + marker + suffix)
  const caret = marked.indexOf(marker)
  return { value: marked.replace(marker, ''), caret }
}
