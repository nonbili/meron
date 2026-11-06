import { useEffect, useState } from 'react'
import { Check, RefreshCw, Search, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { formatFingerprint, sourceLabel } from '../../lib/crypto'
import type { FoundKey, LookupResult } from '../../lib/crypto'
import { addLookedUpKey, crypto$, lookupKey } from '../../states/crypto'
import { showToast } from '../../states/ui'
import { Dialog } from '../dialog/Dialog'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

type Row = {
  addr: string
  state: 'searching' | 'done' | 'error'
  result?: LookupResult
  error?: string
  added?: string
}

/**
 * Looking up recipients' keys, opened by a click (never automatic: the key
 * sources learn who the user writes to). Found keys are shown with where
 * they came from and whether that source checks addresses; nothing is added
 * until the user picks "Use this key".
 */
export function KeyLookupDialog() {
  const { t } = useTranslation()
  const request = useValue(crypto$.lookup)
  const [rows, setRows] = useState<Row[]>([])

  useEffect(() => {
    if (!request) return
    const initial: Row[] = request.addrs.map((addr) => ({ addr, state: 'searching' }))
    setRows(initial)
    let live = true
    for (const addr of request.addrs) {
      void lookupKey(addr)
        .then(
          (result) => live && setRows((rs) => rs.map((r) => (r.addr === addr ? { ...r, state: 'done', result } : r))),
        )
        .catch(
          (err) =>
            live &&
            setRows((rs) => rs.map((r) => (r.addr === addr ? { ...r, state: 'error', error: String(err) } : r))),
        )
    }
    return () => {
      live = false
    }
  }, [request])

  if (!request) return null
  const close = () => crypto$.lookup.set(null)

  const use = async (addr: string, found: FoundKey) => {
    try {
      await addLookedUpKey(addr, found)
      setRows((rs) => rs.map((r) => (r.addr === addr ? { ...r, added: found.info.fingerprint } : r)))
      showToast(t('crypto.lookup.added', { address: addr }), 'success')
    } catch (err) {
      showToast(String(err), 'error')
    }
  }

  const sourceLine = (found: FoundKey) =>
    found.source.kind === 'wkd'
      ? t('crypto.lookup.fromWkd')
      : found.verified
        ? t('crypto.lookup.fromVerified', { server: sourceLabel(found.source) })
        : t('crypto.lookup.fromUnverified', { server: sourceLabel(found.source) })

  return (
    <Dialog onClose={close} aria-label={t('crypto.lookup.title')} className="max-w-lg p-6 flex flex-col gap-5">
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3 min-w-0">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent">
            <Search size={17} />
          </div>
          <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight">{t('crypto.lookup.title')}</h2>
        </div>
        <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={close} />
      </div>

      <div className="flex max-h-[60vh] flex-col gap-5 overflow-y-auto">
        {rows.map((row) => (
          <section key={row.addr} className="flex flex-col gap-2.5" aria-live="polite">
            <h3 className="text-[0.75rem] font-semibold">{row.addr}</h3>
            {row.state === 'searching' && (
              <p className="flex items-center gap-2 text-[0.6875rem] text-secondary">
                <RefreshCw size={11} className="animate-spin" />
                {t('crypto.lookup.searching')}
              </p>
            )}
            {row.state === 'error' && <p className="text-[0.6875rem] text-rose-500">{row.error}</p>}
            {row.state === 'done' && row.result && row.result.found.length === 0 && (
              <p className="text-[0.6875rem] text-secondary">
                {t('crypto.lookup.nothing', { sources: row.result.searched.join(', ') })}
              </p>
            )}
            {row.result?.found.map((found) => {
              const added = row.added === found.info.fingerprint
              return (
                <div key={found.info.fingerprint} className="flex items-center gap-4 rounded-xl bg-hover px-4 py-3.5">
                  <div className="flex min-w-0 flex-1 flex-col gap-1">
                    <div className="truncate text-[0.75rem] font-semibold">{found.info.user_ids[0] ?? row.addr}</div>
                    <div className="font-mono text-[0.625rem] text-secondary">
                      {formatFingerprint(found.info.fingerprint)}
                    </div>
                    <div
                      className={`text-[0.65625rem] leading-snug ${
                        found.verified ? 'text-secondary' : 'text-amber-700 dark:text-amber-300'
                      }`}
                    >
                      {sourceLine(found)}
                    </div>
                  </div>
                  <Button
                    className="shrink-0"
                    variant={added ? 'ghost' : 'primary'}
                    size="sm"
                    disabled={added || !!row.added}
                    onClick={() => void use(row.addr, found)}
                  >
                    {added && <Check size={12} />}
                    <span>{added ? t('crypto.lookup.used') : t('crypto.lookup.use')}</span>
                  </Button>
                </div>
              )
            })}
            {row.result?.updated?.map((key) => (
              <p
                key={`updated-${key.fingerprint}`}
                className={`text-[0.6875rem] ${key.unusable === 'revoked' ? 'text-rose-600 dark:text-rose-400' : 'text-secondary'}`}
              >
                {key.unusable === 'revoked' ? t('crypto.lookup.updatedRevoked') : t('crypto.lookup.updatedRenewed')}
              </p>
            ))}
            {row.result && row.result.failed.length > 0 && (
              <p className="text-[0.625rem] text-secondary">
                {t('crypto.lookup.failed', { sources: row.result.failed.map((f) => f.source).join(', ') })}
              </p>
            )}
          </section>
        ))}
      </div>
      <p className="text-[0.65625rem] leading-relaxed text-secondary">{t('crypto.lookup.note')}</p>
      <div className="flex justify-end">
        <Button variant="secondary" onClick={close}>
          {t('buttons.close')}
        </Button>
      </div>
    </Dialog>
  )
}
