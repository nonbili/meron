// Registry of in-flight / failed optimistic sends, keyed by the temporary
// message id (`sent-…`). Lives in its own module so both compose.ts (which
// dispatches and retries sends) and mail.ts (which deletes messages) can touch
// it without forming an import cycle.
import type { Message } from '../types'

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

/** A reply whose send died before it ever had a payload to retry, and whose
 * draft rescue has not (yet) put a copy on the server. The bubble is the only
 * copy left, which makes it worth putting back when its thread is loaded
 * again: the message page is replaced wholesale on every navigation. `retry`
 * re-runs the rescue (see sendReply's rescueUnsentQuickReply); `inFlight` is
 * set while it runs, and `cancelled` records that the user deleted the bubble
 * meanwhile — the rescue reads it back and undoes what it wrote. */
export type UnsentRescue = {
  threadId: string
  bubble: Message
  retry: () => Promise<void>
  inFlight: boolean
  cancelled: boolean
}

const rescues = new Map<string, UnsentRescue>()

export const setUnsentRescue = (id: string, rescue: UnsentRescue) => {
  rescues.set(id, rescue)
}

export const getUnsentRescue = (id: string) => rescues.get(id)

/** Forget a rescue whose reply is safe on the server. */
export const discardUnsentRescue = (id: string) => {
  rescues.delete(id)
}

/** Forget a rescue because the user deleted its bubble — they threw the reply
 * away, so a rescue still on the wire must undo itself rather than leave the
 * copy (and a resurrected bubble) behind. */
export const cancelUnsentRescue = (id: string) => {
  const rescue = rescues.get(id)
  if (rescue) rescue.cancelled = true
  rescues.delete(id)
}

export const unsentRescues = () => [...rescues.entries()]
