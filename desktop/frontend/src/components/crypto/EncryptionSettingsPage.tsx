import { useCallback, useEffect, useMemo, useState } from 'react'
import { BadgeCheck, FileKey, RefreshCw, Search, Trash2, Upload, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { shortFingerprint } from '../../lib/crypto'
import type { ImportPreview, PeerRow } from '../../lib/crypto'
import { formatFullTimestamp } from '../chat/messageHelpers'
import {
  crypto$,
  importPeers,
  listPeers,
  pickKeyFile,
  previewPeerImport,
  removePeerKey,
  setKeyVerified,
} from '../../states/crypto'
import type { KeyFile } from '../../states/crypto'
import { confirmAction, showToast } from '../../states/ui'
import { Dialog } from '../dialog/Dialog'
import { SettingsGroup, ToggleRow } from '../dialog/AccountSettingsRows'
import { settings$ } from '../../states/settings'
import { KeySourcesSection, LookupRow } from './KeySourcesSection'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

/**
 * Settings → Encryption: everyone's keys in one place. Keys arrive by
 * themselves (Autocrypt, attachments); this is where a user imports an
 * existing keyring, verifies a contact, or removes a key.
 */
export function EncryptionSettingsPage() {
  const { t } = useTranslation()
  const version = useValue(crypto$.version)
  const [peers, setPeers] = useState<PeerRow[] | null>(null)
  const [filter, setFilter] = useState('')
  const [importing, setImporting] = useState<{ file: KeyFile; preview: ImportPreview } | null>(null)
  const [error, setError] = useState('')
  const encryptWhenPossible = useValue(settings$.encryptWhenPossible)

  const reload = useCallback(() => {
    void listPeers()
      .then(setPeers)
      .catch((err) => setError(String(err)))
  }, [])
  useEffect(reload, [reload, version])

  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase()
    return (peers ?? []).filter(
      (p) => !q || p.addr.includes(q) || p.info.user_ids.some((u) => u.toLowerCase().includes(q)),
    )
  }, [peers, filter])

  const startImport = async () => {
    setError('')
    const file = await pickKeyFile(t('crypto.peers.importTitle'))
    if (!file) return
    try {
      setImporting({ file, preview: await previewPeerImport(file) })
    } catch (err) {
      setError(String(err))
    }
  }

  return (
    <div className="flex max-w-2xl flex-col gap-5">
      <div>
        <h2 className="text-base font-bold tracking-tight">{t('crypto.peers.title')}</h2>
        <p className="mt-1 text-[0.71875rem] leading-relaxed text-secondary">{t('crypto.peers.intro')}</p>
      </div>
      <SettingsGroup title={t('crypto.prefs.title')}>
        <ToggleRow
          title={t('crypto.prefs.encryptWhenPossible')}
          hint={t('crypto.prefs.encryptWhenPossibleHint')}
          checked={encryptWhenPossible}
          onChange={() => settings$.encryptWhenPossible.set(!encryptWhenPossible)}
        />
      </SettingsGroup>
      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search size={13} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-secondary" />
          <input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder={t('crypto.peers.filter')}
            aria-label={t('crypto.peers.filter')}
            className="w-full rounded-xl border border-transparent bg-hover py-2 pl-8 pr-3 text-[0.75rem] text-primary placeholder-secondary focus:bg-chats focus:ring-1 focus:ring-accent"
          />
        </div>
        <Button variant="secondary" onClick={() => void startImport()}>
          <Upload size={13} />
          <span>{t('crypto.peers.import')}</span>
        </Button>
      </div>
      {error && (
        <p role="alert" className="px-1 text-[0.6875rem] text-rose-500">
          {error}
        </p>
      )}

      {peers && shown.length === 0 && (
        <p className="rounded-2xl border border-border/60 bg-raised/80 px-4 py-6 text-center text-[0.75rem] text-secondary">
          {peers.length === 0 ? t('crypto.peers.empty') : t('crypto.peers.noMatch')}
        </p>
      )}
      {shown.length > 0 && (
        <ul className="divide-y divide-border/40 overflow-hidden rounded-2xl border border-border/60 bg-raised/80">
          {shown.map((peer) => (
            <PeerRowItem key={`${peer.addr}/${peer.info.fingerprint}`} peer={peer} />
          ))}
        </ul>
      )}

      <LookupRow />
      <KeySourcesSection />

      {importing && (
        <ImportDialog file={importing.file} preview={importing.preview} onClose={() => setImporting(null)} />
      )}
    </div>
  )
}

function PeerRowItem({ peer }: { peer: PeerRow }) {
  const { t } = useTranslation()
  const name = peer.info.user_ids[0]?.replace(/\s*<[^>]*>\s*$/, '') || peer.addr
  return (
    <li className="flex items-center gap-3 px-3.5 py-2.5">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-[0.75rem] font-semibold">{name}</span>
          {peer.verified ? (
            <span className="inline-flex items-center gap-1 rounded-full bg-verified/10 px-1.5 py-0.5 text-[0.59375rem] font-bold text-verified">
              <BadgeCheck size={10} />
              {t('crypto.peers.verified')}
            </span>
          ) : (
            <span className="rounded-full bg-hover px-1.5 py-0.5 text-[0.59375rem] font-bold text-secondary">
              {t('crypto.peers.known')}
            </span>
          )}
          {peer.conflicts_with_verified && (
            <span className="rounded-full bg-rose-500/10 px-1.5 py-0.5 text-[0.59375rem] font-bold text-rose-700 dark:text-rose-300">
              {t('crypto.peers.conflict')}
            </span>
          )}
        </div>
        <div className="truncate text-[0.65625rem] text-secondary">{peer.addr}</div>
        <div className="flex flex-wrap gap-x-2 text-[0.625rem] text-secondary">
          <span className="font-mono">{shortFingerprint(peer.info.fingerprint)}</span>
          <span>· {t(`crypto.peers.source.${peer.source}`)}</span>
          <span title={formatFullTimestamp(peer.last_seen)}>
            · {t('crypto.peers.lastSeen', { date: formatFullTimestamp(peer.last_seen) })}
          </span>
          {peer.info.unusable && (
            <span className="text-amber-700 dark:text-amber-300">
              · {t(`crypto.account.unusable.${peer.info.unusable}`)}
            </span>
          )}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        {peer.verified ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={async () => {
              const ok = await confirmAction({
                title: t('crypto.peers.unverifyTitle'),
                message: t('crypto.peers.unverifyMessage', { sender: name }),
              })
              if (ok) await setKeyVerified(peer.addr, peer.info.fingerprint, '', false)
            }}
          >
            {t('crypto.peers.unverify')}
          </Button>
        ) : (
          <Button
            variant="secondary"
            size="sm"
            onClick={() =>
              crypto$.verify.set({ addr: peer.addr, name, fingerprint: peer.info.fingerprint, accountId: '' })
            }
          >
            <BadgeCheck size={12} />
            <span>{t('crypto.verifyKey')}</span>
          </Button>
        )}
        <IconButton
          icon={Trash2}
          iconSize={13}
          size="sm"
          label={t('crypto.peers.remove')}
          onClick={async () => {
            const ok = await confirmAction({
              title: t('crypto.peers.removeTitle'),
              message: t('crypto.peers.removeMessage', { sender: name }),
              confirmLabel: t('crypto.peers.remove'),
              tone: 'danger',
            })
            if (ok) await removePeerKey(peer.addr, peer.info.fingerprint)
          }}
        />
      </div>
    </li>
  )
}

function ImportDialog({ file, preview, onClose }: { file: KeyFile; preview: ImportPreview; onClose: () => void }) {
  const { t } = useTranslation()
  const [busy, setBusy] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const run = async () => {
    setBusy(true)
    try {
      const n = await importPeers(
        file,
        preview.ready.map((k) => k.fingerprint),
      )
      showToast(
        n > 0 ? t('crypto.peers.imported', { count: n }) : t('crypto.peers.updated', { count: preview.updates.length }),
        'success',
      )
      onClose()
    } catch (err) {
      showToast(String(err), 'error')
      setBusy(false)
    }
  }
  const skippedBy = (reason: string) => preview.skipped.filter((s) => s.reason === reason).length
  return (
    <Dialog
      onClose={onClose}
      dismissible={!busy}
      closeOnBackdrop={false}
      aria-label={t('crypto.peers.importTitle')}
      className="max-w-md p-6 flex flex-col gap-4"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3 min-w-0">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent">
            <FileKey size={17} />
          </div>
          <div className="min-w-0">
            <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight">{t('crypto.peers.importTitle')}</h2>
            <p className="mt-1 truncate text-[0.65625rem] text-secondary">{file.name}</p>
          </div>
        </div>
        <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
      </div>
      <ul className="flex flex-col gap-1 text-[0.75rem]">
        <li className="font-semibold">{t('crypto.peers.ready', { count: preview.ready.length })}</li>
        {preview.already_present.length > 0 && (
          <li className="text-secondary">
            {t('crypto.peers.alreadyPresent', { count: preview.already_present.length })}
          </li>
        )}
        {skippedBy('expired') > 0 && (
          <li className="text-secondary">{t('crypto.peers.skippedExpired', { count: skippedBy('expired') })}</li>
        )}
        {skippedBy('revoked') > 0 && (
          <li className="text-secondary">{t('crypto.peers.skippedRevoked', { count: skippedBy('revoked') })}</li>
        )}
        {preview.skipped.length - skippedBy('expired') - skippedBy('revoked') > 0 && (
          <li className="text-secondary">
            {t('crypto.peers.skippedOther', {
              count: preview.skipped.length - skippedBy('expired') - skippedBy('revoked'),
            })}
          </li>
        )}
        {preview.updates.length > 0 && (
          <li className="text-secondary">{t('crypto.peers.updates', { count: preview.updates.length })}</li>
        )}
        {preview.unreadable > 0 && (
          <li className="text-secondary">{t('crypto.peers.unreadable', { count: preview.unreadable })}</li>
        )}
        {preview.replaces_unverified.length > 0 && (
          <li className="text-secondary">
            {t('crypto.peers.replaces', { addresses: preview.replaces_unverified.join(', ') })}
          </li>
        )}
        {preview.conflicts_with_verified.length > 0 && (
          <li className="text-rose-600 dark:text-rose-400">
            {t('crypto.peers.conflictsVerified', { addresses: preview.conflicts_with_verified.join(', ') })}
          </li>
        )}
      </ul>
      {preview.ready.length > 0 && (
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          className="self-start text-[0.6875rem] font-semibold text-accent cursor-pointer"
        >
          {expanded ? t('crypto.peers.hideList') : t('crypto.peers.showList')}
        </button>
      )}
      {expanded && (
        <ul className="max-h-48 overflow-y-auto rounded-xl bg-hover px-3 py-2 text-[0.6875rem]">
          {preview.ready.map((k) => (
            <li key={k.fingerprint} className="truncate">
              {k.user_ids[0] ?? k.emails[0]}
            </li>
          ))}
        </ul>
      )}
      <p className="px-1 text-[0.65625rem] leading-relaxed text-secondary">{t('crypto.peers.notVerifiedNote')}</p>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onClose} disabled={busy}>
          {t('buttons.cancel')}
        </Button>
        <Button
          variant="primary"
          onClick={() => void run()}
          disabled={busy || (preview.ready.length === 0 && preview.updates.length === 0)}
        >
          {busy && <RefreshCw size={11} className="animate-spin" />}
          <span>
            {preview.ready.length === 0
              ? t('crypto.peers.applyUpdates')
              : t('crypto.peers.importAction', { count: preview.ready.length })}
          </span>
        </Button>
      </div>
    </Dialog>
  )
}
