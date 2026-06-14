// Parse a `mailto:` URL into compose fields. Pure — no app/state dependencies.

export type MailtoFields = {
  to: string
  cc: string
  bcc: string
  subject: string
  body: string
}

function safeDecodeURIComponent(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

/** Parse a `mailto:` URL, or return null when the input isn't a valid mailto. */
export function parseMailto(raw: string): MailtoFields | null {
  let parsed: URL
  try {
    parsed = new URL(raw)
  } catch {
    return null
  }
  if (parsed.protocol.toLowerCase() !== 'mailto:') return null

  const params = parsed.searchParams
  const pathRecipients = safeDecodeURIComponent(parsed.pathname)
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean)
  const queryRecipients = params
    .getAll('to')
    .flatMap((value) => value.split(','))
    .map((part) => part.trim())
    .filter(Boolean)

  return {
    to: [...pathRecipients, ...queryRecipients].join(', '),
    cc: params.get('cc') ?? '',
    bcc: params.get('bcc') ?? '',
    subject: params.get('subject') ?? '',
    body: params.get('body') ?? '',
  }
}
