import { useEffect, useState } from 'react'
import { Download, FileText, FolderOpen } from 'lucide-react'
import { invoke } from './lib/bridge'
import i18n, { resolveI18nLanguageFromWebLocale, useTranslation } from './lib/i18n'
import { hydrateSettings, SETTINGS_DB_KEYS, settings$, watchSystemAppearance } from './states/settings'
import { HtmlMessageView } from './components/chat/HtmlMessageView'
import { formatFullTimestamp } from './components/chat/messageHelpers'
import { useValue } from '@legendapp/state/react'
import type { Attachment } from './types'
import { isMac } from './lib/shortcuts'

type EmlMessage = {
  subject: string
  from_name: string
  from_addr: string
  to: string
  cc: string
  bcc: string
  reply_to: string
  date: number
  body: string
  body_html?: string
  attachments: Attachment[]
}

export function EmlReader({ session }: { session: string }) {
  const { t } = useTranslation()
  const [message, setMessage] = useState<EmlMessage | null>(null)
  const [error, setError] = useState('')
  const [mode, setMode] = useState<'html' | 'plain'>('html')
  const [remote, setRemote] = useState(false)
  const language = useValue(settings$.language)

  useEffect(() => {
    void i18n.changeLanguage(language || resolveI18nLanguageFromWebLocale(navigator.language) || 'en')
  }, [language])

  useEffect(() => {
    let cancelled = false
    void invoke<EmlMessage>('eml.get', { session })
      .then((value) => {
        if (!cancelled) setMessage(value)
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason))
      })
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
        void invoke('eml.close', { session }).catch(console.error)
      }
    }
    const frameKeydown = (event: Event) => {
      const chord = (event as CustomEvent<{ key: string; mod?: boolean; alt?: boolean; shift?: boolean }>).detail
      if (chord.mod && !chord.alt && !chord.shift && chord.key.toLowerCase() === 'w') {
        event.preventDefault()
        void invoke('eml.close', { session }).catch(console.error)
      }
    }
    window.addEventListener('meron.frameKeyDown', frameKeydown)
    window.addEventListener('keydown', keydown)
    return () => {
      cancelled = true
      stopAppearance()
      window.removeEventListener('keydown', keydown)
      window.removeEventListener('meron.frameKeyDown', frameKeydown)
    }
  }, [session])

  const save = async (attachment: Attachment) => {
    try {
      await invoke('eml.saveAttachment', { session, key: attachment.key })
      setError('')
    } catch (reason) {
      setError(String(reason))
    }
  }
  const button = 'rounded-lg px-3 py-1.5 text-xs text-secondary hover:bg-hover hover:text-primary cursor-pointer'

  return (
    <main className="flex h-full flex-col bg-chat text-primary">
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-4 py-2">
        <FileText size={16} className="text-secondary" />
        <span className="min-w-0 flex-1 truncate text-sm font-medium">{message?.subject || t('notify.noSubject')}</span>
        <button
          className={button}
          onClick={() => void invoke('eml.open', { session }).catch((reason) => setError(String(reason)))}
          title={t('eml.open')}
          aria-label={t('eml.open')}
        >
          <FolderOpen size={16} />
        </button>
        {message?.body_html && (
          <>
            <button className={button} aria-pressed={mode === 'html'} onClick={() => setMode('html')}>
              HTML
            </button>
            <button className={button} aria-pressed={mode === 'plain'} onClick={() => setMode('plain')}>
              {t('settings.account.conversationPlain')}
            </button>
            <label className={`${button} flex items-center gap-2`}>
              <input type="checkbox" checked={remote} onChange={(event) => setRemote(event.target.checked)} />
              {t('settings.account.loadRemoteImages')}
            </label>
          </>
        )}
      </div>
      {error && (
        <div role="alert" className="shrink-0 px-5 py-3 text-sm text-red-500">
          {error}
        </div>
      )}
      {message && (
        <HtmlMessageView
          scrollKey={session}
          title={message.subject}
          html={message.body_html}
          text={message.body}
          attachments={message.attachments}
          viewMode={mode}
          allowRemote={remote}
          header={
            <header className="max-w-[760px] space-y-3 px-5 pb-4 pt-6 select-text">
              <h1 className="break-words text-xl font-semibold">{message.subject || t('notify.noSubject')}</h1>
              <dl className="space-y-1 break-words text-sm text-secondary">
                {[
                  [t('composer.fields.from'), [message.from_name, message.from_addr].filter(Boolean).join(' — ')],
                  [t('composer.fields.to'), message.to],
                  [t('composer.fields.cc'), message.cc],
                  [t('composer.fields.bcc'), message.bcc],
                  [t('chat.replyTo'), message.reply_to],
                ]
                  .filter(([, value]) => value)
                  .map(([label, value]) => (
                    <div key={label} className="flex gap-2">
                      <dt className="shrink-0 font-medium">{label}:</dt>
                      <dd>{value}</dd>
                    </div>
                  ))}
              </dl>
              {message.date > 0 && (
                <time className="block text-xs text-secondary">{formatFullTimestamp(message.date)}</time>
              )}
              {message.attachments.length > 0 && (
                <div className="flex flex-wrap gap-2">
                  {message.attachments.map((attachment, index) => (
                    <button
                      key={attachment.key || index}
                      className={`${button} flex max-w-full items-center gap-2 border border-border disabled:opacity-40`}
                      disabled={!attachment.key}
                      onClick={() => void save(attachment)}
                      title={t('chat.saveFile', { filename: attachment.filename })}
                    >
                      <Download size={14} className="shrink-0" />
                      <span className="truncate">{attachment.filename}</span>
                    </button>
                  ))}
                </div>
              )}
            </header>
          }
        />
      )}
    </main>
  )
}
