// Registry of in-flight / failed optimistic sends, keyed by the temporary
// message id (`sent-…`). Lives in its own module so both compose.ts (which
// dispatches and retries sends) and mail.ts (which deletes messages) can touch
// it without forming an import cycle.

/** Args forwarded verbatim to the `mail.send` bridge command. Held here so a
 * failed send can be retried with the original payload — including raw
 * attachment bytes, which the rendered Message drops for non-image files. */
export type PendingSend = {
  account_id: string
  to: string
  cc: string
  subject: string
  body: string
  html?: string
  in_reply_to: string
  references: string
  from: string
  message_id: string
  attachments: { filename: string; mime: string; data: string; inline_id: string }[]
}

/** Prefix of the synthetic id given to optimistically-rendered sent messages.
 * Such ids exist only on the client (the real Sent-folder copy syncs back with
 * a different id), so deletes must stay local rather than hit the backend. */
export const LOCAL_SEND_PREFIX = 'sent-'

export const isLocalSendId = (id: string) => id.startsWith(LOCAL_SEND_PREFIX)

const registry = new Map<string, PendingSend>()

export const setPendingSend = (id: string, payload: PendingSend) => {
  registry.set(id, payload)
}

export const getPendingSend = (id: string) => registry.get(id)

export const discardPendingSend = (id: string) => {
  registry.delete(id)
}
