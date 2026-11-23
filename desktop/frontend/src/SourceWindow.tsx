import { useCallback, useEffect, useRef, useState } from 'react'
import { Check, Copy, Download, LoaderCircle } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { invoke } from './lib/bridge'
import { copyText } from './lib/native'
import i18n, { resolveI18nLanguageFromWebLocale, useTranslation } from './lib/i18n'
import { isMac } from './lib/shortcuts'
import { hydrateSettings, SETTINGS_DB_KEYS, settings$, watchSystemAppearance } from './states/settings'
import { formatFileSize } from './components/chat/messageHelpers'

type MessageSource = {
  /** The whole message's size in bytes. */
  size: number
  /** The message as the server holds it, up to the bridge's display limit. */
  source: string
  truncated: boolean
  /** Some bytes aren't UTF-8 and show as U+FFFD: the text isn't the original. */
  lossy: boolean
  /** From, To, Date, Subject, Message-ID… as present, decoded for reading. */
  headers: { name: string; value: string }[]
  /** How many of the message's bytes `source` holds. */
  shown: number
  /** SPF, DKIM and DMARC verdicts from the topmost Authentication-Results, in
   *  its order: one per DKIM signature, each with the domain it is about. */
  auth: AuthResult[]
  /** Who wrote them, by its own account: normally the receiving server, but
   *  a server that adds none passes on what the sender wrote. */
  authServer: string
}

type AuthResult = { method: string; result: string; domain?: string }

// The "Show original" window: one message's raw source, fetched from the
// server after the window is up, so nothing waits on it but this window. A
// summary of the headers people look a message up for sits above the source.
export function SourceWindow({ session }: { session: string }) {
  const { t } = useTranslation()
  const [source, setSource] = useState<MessageSource | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [copied, setCopied] = useState(false)
  // One save at a time: a second click would open a second dialog, or start
  // a write over the one still under way.
  const [saving, setSaving] = useState(false)
  const copiedTimer = useRef(0)
  const language = useValue(settings$.language)

  useEffect(() => {
    void i18n.changeLanguage(language || resolveI18nLanguageFromWebLocale(navigator.language) || 'en')
  }, [language])

  const load = useCallback(() => {
    let cancelled = false
    setLoading(true)
    setError('')
    void invoke<MessageSource>('source.get', { session })
      .then((value) => {
        if (!cancelled) setSource(value)
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [session])

  useEffect(() => load(), [load])

  useEffect(() => {
    let cancelled = false
    void invoke<{ prefs: Record<string, unknown> }>('app.prefsGet', { keys: SETTINGS_DB_KEYS })
      .then(({ prefs }) => {
        if (!cancelled) hydrateSettings(prefs)
      })
      .catch(console.error)
    const stopAppearance = watchSystemAppearance()
    const keydown = (event: KeyboardEvent) => {
      if (
        (isMac ? event.metaKey : event.ctrlKey) &&
        !event.altKey &&
        !event.shiftKey &&
        event.key.toLowerCase() === 'w'
      ) {
        event.preventDefault()
        void invoke('source.close', { session }).catch(console.error)
      }
    }
    window.addEventListener('keydown', keydown)
    return () => {
      cancelled = true
      stopAppearance()
      window.removeEventListener('keydown', keydown)
      window.clearTimeout(copiedTimer.current)
    }
  }, [session])

  const copy = () => {
    if (!source) return
    void copyText(source.source)
      .then(() => {
        setCopied(true)
        window.clearTimeout(copiedTimer.current)
        copiedTimer.current = window.setTimeout(() => setCopied(false), 1500)
      })
      .catch((reason) => setError(String(reason)))
  }
  const save = () => {
    if (saving) return
    setSaving(true)
    void invoke('source.save', { session })
      .then(() => setError(''))
      .catch((reason) => setError(String(reason)))
      .finally(() => setSaving(false))
  }

  const button =
    'flex h-8 items-center gap-1.5 rounded-full px-3 text-xs font-medium text-secondary cursor-pointer hover:bg-hover hover:text-primary disabled:opacity-40 disabled:cursor-default disabled:hover:bg-transparent'

  return (
    <main className="relative flex h-full flex-col bg-chat text-primary">
      <div className="absolute right-5 top-3 z-10 flex items-center gap-0.5 rounded-full border border-border/40 bg-chats p-1 shadow-float select-none">
        <button type="button" className={button} disabled={!source} onClick={copy}>
          {copied ? <Check size={14} /> : <Copy size={14} />}
          {copied ? t('common.copied') : t('common.copy')}
        </button>
        <button type="button" className={button} disabled={!source || saving} onClick={save}>
          <Download size={14} />
          {t('chat.actions.saveAsEml')}
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-6 pt-16">
        {loading && !source && (
          <div role="status" className="flex items-center gap-2 text-xs text-secondary">
            <LoaderCircle size={14} className="animate-spin" />
            {t('source.loading')}
          </div>
        )}
        {error && (
          <div role="alert" className="mb-4 space-y-2">
            <p className="text-sm font-medium text-primary">{t('source.failed')}</p>
            <p className="break-words text-xs text-secondary select-text">{error}</p>
            {!source && (
              <button
                type="button"
                onClick={load}
                disabled={loading}
                className="rounded-full bg-active border border-border/30 px-4 py-1 text-xs font-medium text-secondary cursor-pointer disabled:opacity-50"
              >
                {t('chat.retry')}
              </button>
            )}
          </div>
        )}
        {source && (
          <>
            <SourceSummary source={source} />
            {source.lossy && <p className="mb-2 text-xs text-secondary">{t('source.lossy')}</p>}
            {source.truncated && (
              <p className="mb-2 text-xs text-secondary">
                {t('source.truncated', {
                  shown: formatFileSize(source.shown),
                  size: formatFileSize(source.size),
                })}
              </p>
            )}
            <pre className="whitespace-pre-wrap break-all rounded-xl border border-border/60 bg-chats p-4 font-mono text-[0.75rem] leading-relaxed text-primary select-text">
              {source.source}
            </pre>
          </>
        )}
      </div>
    </main>
  )
}

// Who sent it and when, and whether the receiving server believed the sender.
function SourceSummary({ source }: { source: MessageSource }) {
  const { t } = useTranslation()
  const verdicts = source.auth
  if (source.headers.length === 0 && verdicts.length === 0) return null
  return (
    <section className="mb-4 rounded-xl border border-border/60 bg-chats p-4 text-xs select-text">
      {source.headers.length > 0 && (
        <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-1.5">
          {source.headers.map((header) => (
            <div key={header.name} className="contents">
              <dt className="font-medium text-secondary">{header.name}</dt>
              <dd className="break-words text-primary">{header.value}</dd>
            </div>
          ))}
        </dl>
      )}
      {verdicts.length > 0 && (
        <div className={`flex flex-wrap items-center gap-2 ${source.headers.length > 0 ? 'mt-3' : ''}`}>
          {verdicts.map((verdict, at) => (
            <AuthVerdict key={at} verdict={verdict} />
          ))}
          {/* The verdicts are only as good as whoever wrote them; say who. */}
          {source.authServer && (
            <span className="text-secondary">{t('source.authBy', { server: source.authServer })}</span>
          )}
        </div>
      )}
    </section>
  )
}

// Neutral whatever the verdict: these are claims copied from the headers, not
// checks Meron made, and a green "pass" would read as one.
function AuthVerdict({ verdict: { method, result: verdict, domain } }: { verdict: AuthResult }) {
  return (
    <span className="rounded-full bg-active px-2.5 py-0.5 font-semibold text-secondary">
      {method.toUpperCase()}
      {domain ? ` (${domain})` : ''}: {verdict}
    </span>
  )
}
