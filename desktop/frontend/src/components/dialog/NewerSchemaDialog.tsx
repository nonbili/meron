import { useEffect, useState } from 'react'
import { AlertTriangle, Loader2 } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { Dialog } from './Dialog'
import { Button } from '../button/Button'
import { invoke } from '../../lib/bridge'
import { useTranslation } from '../../lib/i18n'
import { newerSchema$, openNewerStore } from '../../states/newerSchema'
import { showToast } from '../../states/ui'

// Asked when the store was last used by a newer Meron. Nothing loads until
// the user chooses, so the dialog can't be dismissed: quitting is a choice.
export function NewerSchemaDialog() {
  const { t } = useTranslation()
  const newer = useValue(newerSchema$)
  const [busy, setBusy] = useState<'backup' | 'open' | null>(null)
  const [error, setError] = useState<string | null>(null)
  // A new prompt (the store moved past the schema shown) asks afresh.
  useEffect(() => {
    setBusy(null)
    setError(null)
  }, [newer])

  if (!newer) return null

  const open = async (backup: boolean) => {
    setBusy(backup ? 'backup' : 'open')
    setError(null)
    try {
      const backups = await openNewerStore(backup)
      if (backups.length > 0) {
        showToast(t('store.newer.backedUp', { path: backups.join('\n') }), 'success', 8000)
      }
      // Still busy: the core is opening the store, and `ready` or a new
      // prompt follows.
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      setBusy(null)
    }
  }

  return (
    <Dialog
      onClose={() => {}}
      dismissible={false}
      closeOnBackdrop={false}
      backdropClassName="z-[130] bg-black/45 dark:bg-black/45 backdrop-blur-none"
      role="alertdialog"
      aria-labelledby="newer-schema-title"
      aria-describedby="newer-schema-message"
      className="max-w-md rounded-xl p-4"
    >
      <div className="mb-3 flex items-start gap-3">
        <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-amber-50 text-amber-600 dark:bg-amber-950/30 dark:text-amber-400">
          <AlertTriangle size={17} />
        </div>
        <div className="min-w-0">
          <h2 id="newer-schema-title" className="text-sm font-bold text-primary">
            {t('store.newer.title')}
          </h2>
          <p id="newer-schema-message" className="mt-1 text-sm leading-relaxed text-secondary">
            {t('store.newer.message', { found: newer.found, supported: newer.supported })}
          </p>
          {error && (
            <p role="alert" className="mt-2 break-words text-sm text-rose-600 dark:text-rose-400">
              {t('store.newer.failed', { error })}
            </p>
          )}
        </div>
      </div>
      <div className="flex flex-wrap justify-end gap-2">
        <Button variant="ghost" size="sm" disabled={!!busy} onClick={() => void invoke('app.exit').catch(() => {})}>
          {t('store.newer.quit')}
        </Button>
        <Button variant="secondary" size="sm" disabled={!!busy} onClick={() => void open(false)}>
          {busy === 'open' && <Loader2 size={13} className="animate-spin" />}
          {t('store.newer.openAnyway')}
        </Button>
        <Button autoFocus variant="primary" size="sm" disabled={!!busy} onClick={() => void open(true)}>
          {busy === 'backup' && <Loader2 size={13} className="animate-spin" />}
          {t('store.newer.backupAndOpen')}
        </Button>
      </div>
    </Dialog>
  )
}
