import { Download, Sparkles, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import {
  applyDownloadedUpdate,
  dismissUpdateBanner,
  shouldShowUpdateBanner,
  startUpdateDownload,
  update$,
} from '../../states/update'

// Thin banner announcing a new release. It only appears for a version the user
// hasn't dismissed, and only for the two states where there's something to do:
// a download to start, or a staged update to apply. Everything else about the
// updater lives in About (UpdateSection) so this stays a single action.
export function UpdateBanner() {
  const { t } = useTranslation()
  const status = useValue(update$.status)
  const dismissed = useValue(settings$.dismissedUpdateVersion)

  if (!shouldShowUpdateBanner(status, dismissed)) return null

  const ready = status.state === 'ready'

  return (
    <div className="flex shrink-0 items-center justify-center gap-2 border-b border-accent/20 bg-accent/10 px-3 py-1.5 text-xs font-medium text-primary">
      <Sparkles size={13} className="shrink-0 text-accent" />
      <span className="min-w-0 truncate">
        {ready
          ? t('updates.ready', { version: status.latestVersion })
          : t('updates.available', { version: status.latestVersion })}
      </span>
      <button
        type="button"
        onClick={() => void (ready ? applyDownloadedUpdate() : startUpdateDownload())}
        className="inline-flex h-6 shrink-0 items-center gap-1 rounded-lg px-2 font-semibold text-accent hover:bg-accent/10"
      >
        <Download size={12} />
        <span>{ready ? t('updates.restartAndInstall') : t('updates.download')}</span>
      </button>
      <button
        type="button"
        onClick={() => ui$.aboutOpen.set(true)}
        className="inline-flex h-6 shrink-0 items-center rounded-lg px-2 font-semibold text-secondary hover:bg-hover hover:text-primary"
      >
        {t('updates.details')}
      </button>
      <button
        type="button"
        onClick={dismissUpdateBanner}
        title={t('connectivity.dismiss')}
        aria-label={t('connectivity.dismiss')}
        className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-lg text-secondary hover:bg-hover hover:text-primary"
      >
        <X size={13} />
      </button>
    </div>
  )
}
