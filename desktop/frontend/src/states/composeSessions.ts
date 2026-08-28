type CloseComposeSession = () => Promise<void>

export type ComposeSession = {
  saveChain: Promise<void>
  saveGeneration: number
  savesStopped: boolean
  savedAccountId?: string
  savedDraftMessageId?: string
  close?: CloseComposeSession
  closing?: Promise<void>
}

const sessions = new Map<string, ComposeSession>()

export function getComposeSession(tabId: string): ComposeSession {
  const existing = sessions.get(tabId)
  if (existing) return existing
  const session: ComposeSession = {
    saveChain: Promise.resolve(),
    saveGeneration: 0,
    savesStopped: false,
  }
  sessions.set(tabId, session)
  return session
}

export function registerComposeSession(tabId: string, close: CloseComposeSession) {
  getComposeSession(tabId).close = close
}

export function closeComposeSession(tabId: string, fallback: CloseComposeSession): Promise<void> {
  const session = getComposeSession(tabId)
  if (session.closing) return session.closing
  const run = (session.close ?? fallback)().finally(() => {
    if (session.closing === run) session.closing = undefined
  })
  session.closing = run
  return run
}

export function forgetComposeSession(tabId: string) {
  sessions.delete(tabId)
}

export function pruneComposeSessions(liveTabIds: Set<string>) {
  for (const tabId of sessions.keys()) {
    if (!liveTabIds.has(tabId)) sessions.delete(tabId)
  }
}
