import type { ComposeDraft, ComposerAttachment } from '../types'

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
  /** A message's attachments on their way into the tab. */
  attachmentFetches: Set<Promise<ComposerAttachment[]>>
}

/** A draft the Esc save wrote, and how to bring its closed tab back. */
export type SavedDraft = { draftMessageId: string; reopen: () => void }

const sessions = new Map<string, ComposeSession>()
const draftSaves = new Map<Promise<SavedDraft | null>, ComposeDraft>()

/** Track an Esc save, which outlives its tab. Resolves null when it failed. */
export function trackDraftSave(save: Promise<SavedDraft | null>, draft: ComposeDraft) {
  draftSaves.set(save, draft)
  void save.finally(() => draftSaves.delete(save))
}

/** Whether an Esc save of a draft `matches` picks out is running. */
export function draftSaveRunning(matches: (draft: ComposeDraft) => boolean): boolean {
  return [...draftSaves.values()].some(matches)
}

/**
 * Wait for the Esc saves of the drafts `matches` picks out, and no others: a
 * slow save must not hold up a draft it has nothing to do with.
 */
export async function draftSavesSettled(matches: (draft: ComposeDraft) => boolean): Promise<SavedDraft[]> {
  const pending = [...draftSaves].filter(([, draft]) => matches(draft)).map(([save]) => save)
  const saved = await Promise.all(pending)
  return saved.filter((save) => save !== null)
}

export function getComposeSession(tabId: string): ComposeSession {
  const existing = sessions.get(tabId)
  if (existing) return existing
  const session: ComposeSession = {
    saveChain: Promise.resolve(),
    saveGeneration: 0,
    savesStopped: false,
    attachmentFetches: new Set(),
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
