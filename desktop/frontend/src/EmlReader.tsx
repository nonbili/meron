import { useEffect, useState } from 'react'
import { Download, FolderOpen } from 'lucide-react'
import { invoke } from './lib/bridge'
import { openEmlFile } from './lib/eml'
import i18n, { resolveI18nLanguageFromWebLocale, useTranslation } from './lib/i18n'
import { hydrateSettings, SETTINGS_DB_KEYS, settings$, watchSystemAppearance } from './states/settings'
import { HtmlMessageView } from './components/chat/HtmlMessageView'
import { ReaderCorrespondents } from './components/chat/ReaderCorrespondents'
import { ReaderViewMode } from './components/chat/ReaderViewMode'
import { BlockedRemoteButton } from './components/chat/BlockedRemoteButton'
import { FrostedBand } from './components/chat/FrostedBand'
import { htmlHasRemoteMedia } from './components/chat/messageHelpers'
import { useValue } from '@legendapp/state/react'
import type { Attachment } from './types'
import {
  formatShortcut,
  isBareKeystroke,
  isMac,
  isTyping,
  matchShortcut,
  shortcutForChord,
  type Chord,
} from './lib/shortcuts'

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
  useValue(settings$.shortcutOverrides)
  const openShortcut = formatShortcut('eml.open').join(isMac ? '' : '+')

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
    const open = () => void openEmlFile(session).catch((reason) => setError(String(reason)))
    const keydown = (event: KeyboardEvent) => {
      if (matchShortcut(event) === 'eml.open') {
        if (isBareKeystroke(event) && isTyping(event.target)) return
        event.preventDefault()
        if (event.repeat) return
        open()
        return
      }
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
      const chord = (event as CustomEvent<(Chord & { repeat?: boolean }) | undefined>).detail
      if (!chord?.key) return
      if (shortcutForChord(chord) === 'eml.open') {
        event.preventDefault()
        if (chord.repeat) return
        open()
        return
      }
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
    <main className="relative flex h-full flex-col bg-chat text-primary">
      <FrostedBand fade="bottom" className="left-0 right-3 top-0 h-16" />
      <div className="absolute right-5 top-3 z-10 flex items-center gap-1 rounded-full border border-border/40 bg-chats p-1 shadow-float select-none">
        {message?.body_html && <ReaderViewMode mode={mode} onChange={setMode} />}
        <button
          type="button"
          className="flex h-8 w-8 items-center justify-center rounded-full text-secondary cursor-pointer hover:bg-hover hover:text-primary"
          onClick={() => void openEmlFile(session).catch((reason) => setError(String(reason)))}
          title={`${t('eml.open')}${openShortcut ? ` (${openShortcut})` : ''}`}
          aria-label={t('eml.open')}
        >
          <FolderOpen size={16} />
        </button>
      </div>
      {error && (
        <div role="alert" className="shrink-0 px-5 pb-3 pt-16 text-sm text-red-500">
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
            <header className="max-w-[760px] space-y-3 px-5 pb-4 pt-16 select-text">
              <h1 className="break-words text-xl font-semibold leading-snug text-primary">
                {message.subject || t('notify.noSubject')}
              </h1>
              <ReaderCorrespondents
                showContactAction={false}
                tab={{
                  from: message.from_name || message.from_addr,
                  fromAddr: message.from_addr,
                  fromRaw: message.from_addr
                    ? `${message.from_name ? `${JSON.stringify(message.from_name)} ` : ''}<${message.from_addr}>`
                    : message.from_name,
                  to: message.to,
                  cc: message.cc,
                  bcc: message.bcc,
                  replyTo: message.reply_to,
                  date: message.date,
                }}
                remote={
                  <BlockedRemoteButton
                    blocked={!remote && mode === 'html' && htmlHasRemoteMedia(message.body_html)}
                    hiddenRemoteCount={0}
                    senderAddress=""
                    onReveal={() => setRemote(true)}
                  />
                }
              />
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
