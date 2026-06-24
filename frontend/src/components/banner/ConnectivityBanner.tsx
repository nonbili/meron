import { AlertCircle } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { useValue } from '@legendapp/state/react'
import { connectivity$ } from '../../states/connectivity'

// Thin banner shown across the top when the last sync failed. State is driven by
// sidecar `error` / `mail.synced` events wired up in useAppEffects; this just
// reflects connectivity$. Renders nothing while sync is healthy.
export function ConnectivityBanner() {
  const { t } = useTranslation()
  const error = useValue(connectivity$.error)

  if (!error) return null

  return (
    <div className="flex shrink-0 items-center justify-center gap-2 border-b border-rose-500/20 bg-rose-500/10 px-4 py-1.5 text-xs font-medium text-rose-600 dark:text-rose-400">
      <AlertCircle size={13} />
      <span>{t('connectivity.syncFailed')}</span>
    </div>
  )
}
