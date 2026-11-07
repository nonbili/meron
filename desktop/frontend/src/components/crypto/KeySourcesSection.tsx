import { useEffect, useState } from 'react'
import { Search, Trash2 } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { sourceLabel } from '../../lib/crypto'
import type { KeySource } from '../../lib/crypto'
import { crypto$, keySources, setKeySources } from '../../states/crypto'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { InfoTip } from '../tooltip/InfoTip'

const input =
  'flex-1 rounded-xl border border-transparent bg-hover px-3 py-2 text-[0.75rem] text-primary placeholder-secondary focus:bg-chats focus:ring-1 focus:ring-accent'

/** Look one address up by hand (the same dialog the composer opens). */
export function LookupRow() {
  const { t } = useTranslation()
  const [addr, setAddr] = useState('')
  const valid = /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(addr.trim())
  const go = () => {
    if (valid) crypto$.lookup.set({ addrs: [addr.trim().toLowerCase()] })
  }
  return (
    <section className="flex flex-col gap-2">
      <h3 className="px-1 text-[0.75rem] font-semibold text-secondary">{t('crypto.lookup.title')}</h3>
      <div className="flex items-center gap-2">
        <input
          value={addr}
          onChange={(e) => setAddr(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') go()
          }}
          type="email"
          placeholder="name@example.com"
          aria-label={t('crypto.lookup.address')}
          className={input}
        />
        <Button variant="secondary" disabled={!valid} onClick={go}>
          <Search size={13} />
          <span>{t('crypto.lookup.action')}</span>
        </Button>
      </div>
    </section>
  )
}

/**
 * Where lookups go: WKD and keys.openpgp.org by default (both check that a
 * key belongs to its address); more keyservers can be added and are marked
 * as not checking addresses.
 */
export function KeySourcesSection() {
  const { t } = useTranslation()
  const [sources, setSources] = useState<KeySource[] | null>(null)
  const [defaults, setDefaults] = useState<KeySource[]>([])
  const [adding, setAdding] = useState('')
  const [error, setError] = useState('')

  useEffect(() => {
    void keySources()
      .then((r) => {
        setSources(r.sources)
        setDefaults(r.defaults)
      })
      .catch((err) => setError(String(err)))
  }, [])

  const save = async (next: KeySource[]) => {
    setError('')
    try {
      setSources(await setKeySources(next))
    } catch (err) {
      setError(String(err))
    }
  }
  if (!sources) return null
  const add = async () => {
    const raw = adding.trim()
    if (!raw) return
    const url = raw.startsWith('https://') ? raw : `https://${raw.replace(/^hkps?:\/\//, '')}`
    await save([...sources, { kind: 'hkp', url, enabled: true }])
    setAdding('')
  }
  const isDefault = (s: KeySource) => defaults.some((d) => d.kind === s.kind && d.url === s.url)

  return (
    <section className="flex flex-col gap-2">
      <h3 className="px-1 text-[0.75rem] font-semibold text-secondary">{t('crypto.sources.title')}</h3>
      <ul className="divide-y divide-border/40 overflow-hidden rounded-2xl border border-border/60 bg-raised/80">
        {sources.map((source, i) => (
          <li key={`${source.kind}:${source.url}`} className="flex items-center gap-3 px-3.5 py-2">
            <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-2.5">
              <input
                type="checkbox"
                checked={source.enabled}
                onChange={() => void save(sources.map((s, j) => (j === i ? { ...s, enabled: !s.enabled } : s)))}
                className="cursor-pointer accent-accent"
              />
              <span className="min-w-0">
                <span className="flex min-w-0 items-center gap-1.5">
                  <span className="truncate text-[0.75rem] font-semibold">{sourceLabel(source)}</span>
                  {isDefault(source) && <InfoTip hint={t(`crypto.sources.why.${source.kind}`)} />}
                </span>
                <span
                  className={`block text-[0.625rem] ${
                    source.kind === 'hkp' ? 'text-amber-700 dark:text-amber-300' : 'text-secondary'
                  }`}
                >
                  {t(`crypto.sources.kind.${source.kind}`)}
                </span>
              </span>
            </label>
            {!isDefault(source) && (
              <IconButton
                icon={Trash2}
                iconSize={13}
                size="sm"
                label={t('crypto.sources.remove')}
                onClick={() => void save(sources.filter((_, j) => j !== i))}
              />
            )}
          </li>
        ))}
      </ul>
      <div className="flex items-center gap-2">
        <input
          value={adding}
          onChange={(e) => setAdding(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void add()
          }}
          placeholder="keyserver.ubuntu.com"
          aria-label={t('crypto.sources.add')}
          className={input}
        />
        <Button variant="secondary" disabled={!adding.trim()} onClick={() => void add()}>
          {t('crypto.sources.add')}
        </Button>
        <Button variant="ghost" onClick={() => void save(defaults)}>
          {t('crypto.sources.reset')}
        </Button>
      </div>
      {error && (
        <p role="alert" className="px-1 text-[0.6875rem] text-rose-500">
          {error}
        </p>
      )}
    </section>
  )
}
