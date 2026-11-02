type CloseComposeSession = () => Promise<void>

export type ComposeSession = {
  saveChain: Promise<void>
  saveGeneration: number
  savesStopped: boolean
  savedAccountId?: string
  savedDraftMessageId?: string
  close?: CloseComposeSession
  /** Closes the tab but keeps the message in Drafts (Esc). */
  closeKeepingDraft?: CloseComposeSession
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

export function registerComposeSession(
  tabId: string,
  close: CloseComposeSession,
  closeKeepingDraft?: CloseComposeSession,
) {
  const session = getComposeSession(tabId)
  session.close = close
  session.closeKeepingDraft = closeKeepingDraft
}

function runClose(session: ComposeSession, close: CloseComposeSession): Promise<void> {
  if (session.closing) return session.closing
  const run = close().finally(() => {
    if (session.closing === run) session.closing = undefined
  })
  session.closing = run
  return run
}

export function closeComposeSession(tabId: string, fallback: CloseComposeSession): Promise<void> {
  const session = getComposeSession(tabId)
  return runClose(session, session.close ?? fallback)
}

/**
 * Esc in a compose tab: close it, keeping the message in Drafts. Only a
 * mounted composer can do that (it owns the last save), so for a tab without
 * one this returns false and leaves the key to others. Shares `closing` with
 * the × so the two never race.
 */
export function closeComposeSessionKeepingDraft(tabId: string): boolean {
  const session = sessions.get(tabId)
  if (!session?.closeKeepingDraft) return false
  void runClose(session, session.closeKeepingDraft)
  return true
}

export function forgetComposeSession(tabId: string) {
  sessions.delete(tabId)
}

export function pruneComposeSessions(liveTabIds: Set<string>) {
  for (const tabId of sessions.keys()) {
    if (!liveTabIds.has(tabId)) sessions.delete(tabId)
  }
}
