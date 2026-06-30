import { useState } from 'react'
import { AlertCircle, RefreshCw, X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { useValue } from '@legendapp/state/react'
import { invoke } from '../../lib/bridge'
import { accounts$ } from '../../states/accounts'
import { connectivity$, clearSyncErrorFor, dismissSyncError } from '../../states/connectivity'
import { syncMail } from '../../states/mail'
import { showToast } from '../../states/ui'

// Thin banner shown across the top when the last sync failed. State is driven by
// sidecar `error` / `mail.synced` events wired up in useAppEffects; this just
// reflects connectivity$. Renders nothing while sync is healthy.
export function ConnectivityBanner() {
  const { t } = useTranslation()
  const error = useValue(connectivity$.error)
  const accountId = useValue(connectivity$.account)
  const accounts = useValue(accounts$)
  const [retrying, setRetrying] = useState(false)

  if (!error) return null

  const account = accountId ? accounts.find((acc) => acc.id === accountId) : null
  const accountLabel = account ? account.display_name || account.email : accountId

  const retry = async () => {
    if (retrying) return
    setRetrying(true)
    try {
      if (accountId) {
        await invoke('mail.sync', { account_id: accountId })
        clearSyncErrorFor(accountId)
        showToast(t('connectivity.syncedAccount', { account: accountLabel ?? accountId }))
      } else {
        await syncMail()
      }
    } catch (err) {
      showToast(err instanceof Error ? err.message : t('connectivity.retryFailed'), 'error')
    } finally {
      setRetrying(false)
    }
  }

  return (
    <div className="flex shrink-0 items-center justify-center gap-2 border-b border-rose-500/20 bg-rose-500/10 px-3 py-1.5 text-xs font-medium text-rose-600 dark:text-rose-400">
      <AlertCircle size={13} className="shrink-0" />
      <span className="min-w-0 truncate">
        {accountLabel ? t('connectivity.syncFailedAccount', { account: accountLabel }) : t('connectivity.syncFailed')}
      </span>
      <button
        type="button"
        onClick={retry}
        disabled={retrying}
        className="inline-flex h-6 shrink-0 items-center gap-1 rounded-lg px-2 font-semibold text-rose-700 hover:bg-rose-500/10 disabled:opacity-60 dark:text-rose-300"
      >
        <RefreshCw size={12} className={retrying ? 'animate-spin' : ''} />
        <span>{t('connectivity.retry')}</span>
      </button>
      <button
        type="button"
        onClick={dismissSyncError}
        title={t('connectivity.dismiss')}
        aria-label={t('connectivity.dismiss')}
        className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-lg text-rose-700 hover:bg-rose-500/10 dark:text-rose-300"
      >
        <X size={13} />
      </button>
    </div>
  )
}
