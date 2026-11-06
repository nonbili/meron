import { observable } from '@legendapp/state'

// Separate webviews share localStorage. Keep the tray editor's recovery buffer
// away from the inbox's tabs and navigation, including during module loading.
export const standaloneCompose = new URLSearchParams(window.location.search).get('compose') === 'standalone'
export const STANDALONE_COMPOSE_KEY = 'meron-standalone-compose-tabs'

export const externalComposeDraft$ = observable('')

// A recovery buffer may outlive its webview. Once its draft is taken over,
// sent or discarded in the inbox, it must never be restored by a later tray click.
export function forgetStandaloneRecoveryDraft(draftId: string | undefined, accountId?: string) {
  if (standaloneCompose || !draftId) return
  const normalize = (value: string) => value.trim().replace(/^<|>$/g, '').toLowerCase()
  try {
    const tabs = JSON.parse(localStorage.getItem(STANDALONE_COMPOSE_KEY) || '[]')
    if (!Array.isArray(tabs)) return
    const remaining = tabs.filter((tab) => {
      const draft = tab?.compose
      return (
        typeof draft?.draftMessageId !== 'string' ||
        normalize(draft.draftMessageId) !== normalize(draftId) ||
        (accountId && draft.accountId !== accountId)
      )
    })
    if (remaining.length !== tabs.length) localStorage.setItem(STANDALONE_COMPOSE_KEY, JSON.stringify(remaining))
  } catch {
    /* An invalid recovery buffer cannot own a draft. */
  }
}
