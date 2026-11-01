/** The search options form, as the core's `search.parse` reads a query. */
export type SearchFields = {
  from: string
  to: string
  subject: string
  /** Separated by spaces or commas; a thread must carry them all. */
  tags: string
  words: string
  /** `yyyy-mm-dd`, or empty. */
  after: string
  /** `yyyy-mm-dd`, or empty. */
  before: string
  unread: boolean
  starred: boolean
  has_attachment: boolean
}

/** Whether nothing is filled in. */
export function fieldsEmpty(fields: SearchFields) {
  return (
    !fields.unread &&
    !fields.starred &&
    !fields.has_attachment &&
    [fields.from, fields.to, fields.subject, fields.tags, fields.words, fields.after, fields.before].every(
      (value) => !value.trim(),
    )
  )
}

export const EMPTY_FIELDS: SearchFields = {
  from: '',
  to: '',
  subject: '',
  tags: '',
  words: '',
  after: '',
  before: '',
  unread: false,
  starred: false,
  has_attachment: false,
}
