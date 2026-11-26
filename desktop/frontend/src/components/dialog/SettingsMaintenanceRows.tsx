import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from '../../lib/i18n'
import { Dialog } from './Dialog'
import { X, Upload, Download, Trash2, ScrollText, Archive } from 'lucide-react'
import { exportBackup, importBackup, backupErrorMessage, isWrongPassphrase } from '../../states/backup'
import { BackupPassphraseDialog, type BackupPassphraseMode } from './BackupPassphraseDialog'
import { showToast } from '../../states/ui'
import { accounts$ } from '../../states/accounts'
import { IconButton } from '../button/IconButton'
import { SettingRow } from './AccountSettingsRows'
import { invoke } from '../../lib/bridge'

// Backup / restore of the app's configuration. Cached mail is not included, so
// a restored account re-syncs rather than arriving with its history.
//
// Both buttons hand off to a passphrase dialog: exporting to offer encryption,
// restoring only when the chosen file turns out to be encrypted (which the
// bridge reports back with the path, so the file dialog isn't shown twice).
export function SettingsBackupRow() {
  const { t } = useTranslation()
  const [prompt, setPrompt] = useState<BackupPassphraseMode | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  // The file already chosen for a restore that came back needing a passphrase.
  const [pendingPath, setPendingPath] = useState('')

  const closePrompt = () => {
    setPrompt(null)
    setError('')
    setPendingPath('')
  }

  const runExport = async (passphrase: string, includeSecrets: boolean) => {
    setBusy(true)
    setError('')
    try {
      const saved = await exportBackup(includeSecrets, passphrase)
      closePrompt()
      if (saved) showToast(t('settings.backup.exported'), 'success')
    } catch (err) {
      setError(backupErrorMessage(err, t('settings.backup.exportFailed')))
    } finally {
      setBusy(false)
    }
  }

  // `path` is empty on the first attempt (the bridge opens a file dialog) and
  // set on the retry after a passphrase prompt.
  const runImport = async (path: string, passphrase: string) => {
    setBusy(true)
    setError('')
    try {
      const outcome = await importBackup(path, passphrase)
      if (outcome.status === 'cancelled') {
        closePrompt()
        return
      }
      if (outcome.status === 'needs-passphrase') {
        setPendingPath(outcome.path)
        setPrompt('restore')
        return
      }
      closePrompt()
      const { accounts, skipped, openpgpMissingSecret } = outcome.summary
      if (openpgpMissingSecret.length > 0) {
        // More pressing than the count: encrypted mail of these accounts
        // stays unreadable until their key is brought in.
        const all = accounts$.get()
        const names = openpgpMissingSecret.map((id) => all.find((acc) => acc.id === id)?.email || id)
        showToast(t('settings.backup.missingEncryptionKey', { accounts: names.join(', ') }), 'error')
      } else if (accounts === 0 && skipped > 0) {
        showToast(t('settings.backup.restoredNothingNew'), 'success')
      } else {
        showToast(t('settings.backup.restored', { count: accounts }), 'success')
      }
    } catch (err) {
      const message = backupErrorMessage(err, t('settings.backup.restoreFailed'))
      // A wrong passphrase keeps the prompt open so the user can retype it;
      // anything else is a real failure and closes it.
      if (isWrongPassphrase(message) && pendingPath) {
        setError(t('settings.backup.wrongPassphrase'))
      } else {
        closePrompt()
        showToast(message, 'error')
      }
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      <SettingRow
        title={t('settings.backup.fileTitle')}
        hint={t('settings.backup.fileHint')}
        control={
          <div className="flex items-center gap-2">
            <button
              onClick={() => runImport('', '')}
              disabled={busy}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Upload size={12} />
              {t('settings.backup.restoreAction')}
            </button>
            <button
              onClick={() => {
                setError('')
                setPrompt('export')
              }}
              disabled={busy}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Archive size={12} />
              {t('common.export')}
            </button>
          </div>
        }
      />

      {/* Portalled: the row sits in a divided card, whose dividers would
          otherwise land on the overlay too. */}
      {prompt &&
        createPortal(
          <BackupPassphraseDialog
            mode={prompt}
            busy={busy}
            error={error}
            onCancel={closePrompt}
            onSubmit={(passphrase, includeSecrets) =>
              prompt === 'export' ? runExport(passphrase, includeSecrets) : runImport(pendingPath, passphrase)
            }
          />,
          document.body,
        )}
    </>
  )
}

type StorageUsage = { cacheBytes: number; dbBytes: number; archiveBytes?: number }

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const value = bytes / 1024 ** i
  return `${value >= 100 || i === 0 ? Math.round(value) : value.toFixed(1)} ${units[i]}`
}

export function SettingsStorageRows() {
  const { t } = useTranslation()
  const [usage, setUsage] = useState<StorageUsage | null>(null)
  const [clearing, setClearing] = useState(false)
  // Two-step guard: the first click arms "Confirm?", the second actually clears.
  // Prevents wiping the cache from a stray click. Auto-disarms after a few seconds.
  const [confirming, setConfirming] = useState(false)

  useEffect(() => {
    let alive = true
    invoke<StorageUsage>('storage.usage')
      .then((u) => {
        if (alive) setUsage(u)
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [])

  useEffect(() => {
    if (!confirming) return
    const id = setTimeout(() => setConfirming(false), 4000)
    return () => clearTimeout(id)
  }, [confirming])

  const clearCache = async () => {
    if (!confirming) {
      setConfirming(true)
      return
    }
    setConfirming(false)
    setClearing(true)
    try {
      const u = await invoke<StorageUsage>('storage.clearCache')
      setUsage(u)
      showToast(t('settings.storage.clearedToast'), 'success')
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), 'error')
    } finally {
      setClearing(false)
    }
  }

  return (
    <>
      <SettingRow
        title={t('settings.storage.usageTitle')}
        hint={t('settings.storage.usageHint')}
        control={
          <span className="text-xs text-secondary tabular-nums">
            {usage
              ? `${t('settings.storage.cacheLabel')} ${formatBytes(usage.cacheBytes)} · ${t('settings.storage.databaseLabel')} ${formatBytes(usage.dbBytes)}${
                  usage.archiveBytes
                    ? ` · ${t('settings.sections.localFolders')} ${formatBytes(usage.archiveBytes)}`
                    : ''
                }`
              : '…'}
          </span>
        }
      />
      <SettingRow
        title={t('settings.storage.clearTitle')}
        hint={t('settings.storage.clearHint')}
        control={
          <button
            onClick={clearCache}
            disabled={clearing || (usage?.cacheBytes ?? 0) === 0}
            className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl font-bold text-[0.625rem] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${
              confirming ? 'bg-red-500 hover:bg-red-600 text-white' : 'bg-hover hover:bg-active text-primary'
            }`}
          >
            <Trash2 size={12} />
            {confirming ? t('settings.storage.clearConfirm') : t('settings.storage.clearButton')}
          </button>
        }
      />
    </>
  )
}

export function SettingsLogsRow() {
  const { t } = useTranslation()
  const [viewerOpen, setViewerOpen] = useState(false)
  return (
    <>
      <SettingRow
        title={t('settings.sections.logs')}
        hint={t('settings.syncDiagnosticLogHint')}
        control={
          <button
            onClick={() => setViewerOpen(true)}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
          >
            <ScrollText size={12} />
            {t('settings.viewSyncLog')}
          </button>
        }
      />
      {viewerOpen && createPortal(<LogViewerDialog onClose={() => setViewerOpen(false)} />, document.body)}
    </>
  )
}

// In-app viewer for the local app log, so the user can inspect what would be
// shared before exporting it (export lives in the dialog header).
function LogViewerDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation()
  const [log, setLog] = useState<string | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  // The newest entries are at the end; start there.
  useEffect(() => {
    const el = scrollRef.current
    if (el && log) el.scrollTop = el.scrollHeight
  }, [log])

  useEffect(() => {
    let alive = true
    invoke<{ log: string }>('log.read')
      .then((res) => {
        if (alive) setLog(res?.log ?? '')
      })
      .catch((error) => {
        showToast(error instanceof Error ? error.message : String(error), 'error')
        if (alive) setLog('')
      })
    return () => {
      alive = false
    }
  }, [])

  const exportLog = async () => {
    try {
      const res = await invoke<{ saved: boolean; path?: string }>('log.export')
      if (res?.saved) showToast(t('settings.toast.logExported'), 'success')
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), 'error')
    }
  }

  return (
    <Dialog
      onClose={onClose}
      aria-label={t('settings.viewSyncLog')}
      backdropClassName="bg-black/35"
      className="border-border/80 max-w-3xl h-[560px] max-h-[85vh] shadow-black/20 dark:shadow-black/45 flex flex-col overflow-hidden"
    >
      <div className="flex items-center justify-between gap-4 px-6 py-4.5 border-b border-border/60 shrink-0 bg-chats/95">
        <h2 className="text-base font-bold tracking-tight leading-tight">{t('settings.viewSyncLog')}</h2>
        <div className="flex items-center gap-2">
          <button
            onClick={() => void exportLog()}
            disabled={!log}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            <Download size={12} />
            {t('common.export')}
          </button>
          <IconButton icon={X} iconSize={16} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>
      </div>
      <div ref={scrollRef} className="flex-1 min-h-0 overflow-y-auto p-4 select-text">
        {log === null ? (
          <p className="text-xs text-secondary">{t('common.loading')}</p>
        ) : log === '' ? (
          <p className="text-xs text-secondary">{t('settings.syncLogEmpty')}</p>
        ) : (
          <pre className="whitespace-pre-wrap break-all font-mono text-[0.6875rem] leading-4 text-primary">{log}</pre>
        )}
      </div>
    </Dialog>
  )
}
