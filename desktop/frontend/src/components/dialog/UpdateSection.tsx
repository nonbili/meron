import { AlertCircle, CheckCircle2, Download, ExternalLink, Loader2, RefreshCw, Sparkles } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openExternal } from '../../lib/native'
import { update$, applyDownloadedUpdate, runUpdateCheck, startUpdateDownload } from '../../states/update'
import { Button } from '../button/Button'

function formatMegabytes(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1)
}

/**
 * The updater's one control surface, shown inside AboutDialog. Every state the
 * Go side can be in maps to exactly one row here, so there is never a moment
 * where the user can't tell what the app is doing with an update.
 */
export function UpdateSection() {
  const { t } = useTranslation()
  const status = useValue(update$.status)

  // Store builds (Snap, Flathub, Microsoft Store) point at the right place for
  // updates. Anything else that can't self-update — `wails dev`, a binary run
  // from an unrecognized location — says nothing at all rather than explaining
  // a limitation the user can't act on.
  if (!status.supported) {
    if (!status.managed) return null
    return (
      <div className="mt-5 w-full rounded-2xl border border-border/70 bg-raised/70 px-4 py-3 text-xs text-secondary">
        {t('updates.managedExternally')}
      </div>
    )
  }

  const percent = status.total > 0 ? Math.min(100, Math.round((status.downloaded / status.total) * 100)) : 0

  return (
    <div className="mt-5 w-full rounded-2xl border border-border/70 bg-raised/70 px-4 py-3">
      {status.state === 'checking' && (
        <div className="flex items-center justify-center gap-2 text-xs font-semibold text-secondary">
          <Loader2 size={14} className="animate-spin" />
          <span>{t('updates.checking')}</span>
        </div>
      )}

      {status.state === 'idle' && (
        <div className="flex items-center justify-between gap-2">
          <span className="flex items-center gap-2 text-xs font-semibold text-secondary">
            <CheckCircle2 size={14} className="text-emerald-500" />
            {t('updates.upToDate')}
          </span>
          <Button variant="secondary" size="sm" leftIcon={RefreshCw} onClick={() => void runUpdateCheck()}>
            {t('updates.check')}
          </Button>
        </div>
      )}

      {status.state === 'available' && (
        <div className="flex items-center justify-between gap-2">
          <span className="flex min-w-0 items-center gap-2 text-xs font-semibold text-primary">
            <Sparkles size={14} className="text-accent" />
            <span className="truncate">{t('updates.available', { version: status.latestVersion })}</span>
          </span>
          <Button variant="primary" size="sm" leftIcon={Download} onClick={() => void startUpdateDownload()}>
            {t('updates.download')}
          </Button>
        </div>
      )}

      {status.state === 'downloading' && (
        <div>
          <div className="flex items-center justify-between gap-2 text-xs font-semibold text-secondary">
            <span>{t('updates.downloading', { version: status.latestVersion })}</span>
            <span className="tabular-nums">
              {formatMegabytes(status.downloaded)} / {formatMegabytes(status.total)} MB
            </span>
          </div>
          <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-border">
            <div className="h-full rounded-full bg-accent transition-[width]" style={{ width: `${percent}%` }} />
          </div>
        </div>
      )}

      {(status.state === 'ready' || status.state === 'installing') && (
        <div>
          <div className="flex items-center justify-between gap-2">
            <span
              className={`flex min-w-0 items-center gap-2 text-xs font-semibold ${
                status.error ? 'text-rose-600 dark:text-rose-400' : 'text-primary'
              }`}
            >
              {status.error ? (
                <AlertCircle size={14} className="shrink-0" />
              ) : (
                <Download size={14} className="text-accent" />
              )}
              <span className="truncate">
                {status.error ? t('updates.failed') : t('updates.ready', { version: status.latestVersion })}
              </span>
            </span>
            <Button
              variant="primary"
              size="sm"
              disabled={status.state === 'installing'}
              onClick={() => void applyDownloadedUpdate()}
            >
              {status.state === 'installing' ? t('updates.installing') : t('updates.restartAndInstall')}
            </Button>
          </div>
          {status.error && (
            <p className="mt-2 break-words text-[0.6875rem] leading-4 text-rose-600 dark:text-rose-400">
              {status.error}
            </p>
          )}
        </div>
      )}

      {status.state === 'error' && (
        <div className="flex items-center justify-between gap-2">
          <span className="flex min-w-0 items-center gap-2 text-xs font-semibold text-rose-600 dark:text-rose-400">
            <AlertCircle size={14} className="shrink-0" />
            <span className="truncate" title={status.error}>
              {t('updates.failed')}
            </span>
          </span>
          <Button variant="secondary" size="sm" leftIcon={RefreshCw} onClick={() => void runUpdateCheck()}>
            {t('updates.retry')}
          </Button>
        </div>
      )}

      {/* The installer channels can all fail on a read-only or root-owned
          install dir, so a manual route out is always one click away. */}
      {(status.state === 'error' || status.state === 'ready' || status.state === 'installing') &&
        status.releasesUrl && (
          <button
            type="button"
            className="mt-2 inline-flex items-center gap-1 text-[0.6875rem] font-semibold text-secondary hover:text-primary"
            onClick={() => openExternal(status.releasesUrl)}
          >
            {t('updates.downloadManually')}
            <ExternalLink size={11} />
          </button>
        )}

      {status.state === 'ready' && status.channel === 'nsis' && (
        <p className="mt-2 text-[0.6875rem] leading-4 text-secondary">{t('updates.windowsPermissionHint')}</p>
      )}
    </div>
  )
}
