import { useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { Composer } from './components/composer/Composer'
import { AppConfirm } from './components/dialog/AppConfirm'
import { AppToast } from './components/toast/AppToast'
import { invoke, invokeRetryingTimeout } from './lib/bridge'
import i18n, { resolveI18nLanguageFromWebLocale, useTranslation } from './lib/i18n'
import { isMac } from './lib/shortcuts'
import { accounts$ } from './states/accounts'
import { openComposeTab, updateComposeDraft } from './states/compose'
import { compose$, newDraftMessageId } from './states/composeState'
import { closeComposeSessionKeepingDraft } from './states/composeSessions'
import { hydrateSettings, SETTINGS_DB_KEYS, settings$, watchSystemAppearance } from './states/settings'
import type { Account } from './types'

// Deliberately avoid boot(): it restores inbox navigation, claims preference
// write ownership, drains mailto links and prunes media belonging to other windows.
export function ComposeWindow() {
  const { t } = useTranslation()
  const [tabId, setTabId] = useState('')
  const [error, setError] = useState('')
  const startup = useRef<Promise<string | undefined> | null>(null)
  const initialized = useRef(false)
  const tabs = useValue(compose$.tabs)
  const language = useValue(settings$.language)
  const tab = tabs.find((candidate) => candidate.id === tabId)

  useEffect(() => {
    void i18n.changeLanguage(language || resolveI18nLanguageFromWebLocale(navigator.language) || 'en')
  }, [language])

  useEffect(() => {
    let disposed = false
    const refresh = async () => {
      const [accounts, { prefs }] = await Promise.all([
        invokeRetryingTimeout<{ accounts: Account[] }>('account.list'),
        invokeRetryingTimeout<{ prefs: Record<string, unknown> }>('app.prefsGet', { keys: SETTINGS_DB_KEYS }),
      ])
      hydrateSettings(prefs)
      accounts$.set(accounts.accounts)
    }
    // Keep one initialization across StrictMode's effect replay.
    const restore = async () => {
      const recovered = compose$.tabs.peek()[0]
      if (recovered?.compose) {
        const claimed = await invoke<boolean>('composer.claimDraft', { draft_id: recovered.compose.draftMessageId })
        if (!claimed) {
          // The inbox already owns this server draft. Preserve the recovery
          // buffer as a separate draft so neither editor can overwrite it.
          updateComposeDraft(recovered.id, { draftMessageId: newDraftMessageId(), sourceDraft: undefined })
        }
        return recovered.id
      }
      return openComposeTab()
    }
    const initialize = () => {
      startup.current ??= refresh()
        .then(restore)
        .then(
          (id) => {
            if (!id) startup.current = null
            return id
          },
          (reason) => {
            startup.current = null
            throw reason
          },
        )
      return startup.current
    }
    const show = (id: string | undefined) => {
      if (disposed) return
      if (id) {
        initialized.current = true
        setTabId(id)
        setError('')
      } else setError(t('compose.toast.addMailAccountBeforeComposing'))
    }
    const reportError = (reason: unknown) => {
      if (!disposed) setError(String(reason))
    }
    void initialize().then(show).catch(reportError)
    const refreshOnFocus = () => {
      // Focus/configuration events during a slow boot join the same startup;
      // after a failed/no-account boot they may start a fresh attempt.
      if (!initialized.current) void initialize().then(show).catch(reportError)
      else void refresh().catch(console.error)
    }
    const stopAppearance = watchSystemAppearance()
    const eventsOn = (window as any).runtime?.EventsOn
    const stopConfiguration = eventsOn?.('configuration.changed', refreshOnFocus)
    window.addEventListener('focus', refreshOnFocus)
    return () => {
      disposed = true
      stopAppearance()
      stopConfiguration?.()
      window.removeEventListener('focus', refreshOnFocus)
    }
  }, [])

  useEffect(() => {
    const close = () => {
      if (!tabId || !closeComposeSessionKeepingDraft(tabId)) void invoke('composer.closeWindow').catch(console.error)
    }
    const stop = (window as any).runtime?.EventsOn?.('composer.closeRequested', (detail?: { request?: number }) => {
      void invoke('composer.closeResponded', { request: detail?.request }).catch(console.error)
      close()
    })
    const keydown = (event: KeyboardEvent) => {
      if (
        (isMac ? event.metaKey : event.ctrlKey) &&
        !event.altKey &&
        !event.shiftKey &&
        event.key.toLowerCase() === 'w'
      ) {
        event.preventDefault()
        close()
      }
    }
    window.addEventListener('keydown', keydown)
    return () => {
      stop?.()
      window.removeEventListener('keydown', keydown)
    }
  }, [tabId])

  useEffect(() => {
    if (!tabId) return
    if (!tab) {
      void invoke('composer.closeWindow').catch(console.error)
      return
    }
    document.title = `${tab.compose?.subject.trim() || t('composer.actions.newMessage')} — Meron`
  }, [tabId, tab, t])

  const draftId = tab?.compose?.draftMessageId
  useEffect(() => {
    if (draftId) void invoke('composer.setDraft', { draft_id: draftId }).catch(console.error)
  }, [draftId])

  return (
    <main className="relative flex h-full flex-col bg-chat text-primary">
      {error && (
        <div role="alert" className="p-5 text-sm text-red-500">
          {error}
        </div>
      )}
      {tab && <Composer tabId={tab.id} />}
      <AppConfirm />
      <AppToast />
    </main>
  )
}
