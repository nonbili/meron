import { useCallback, useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { Composer } from './components/composer/Composer'
import { MessagePane } from './components/chat/MessagePane'
import { AppConfirm } from './components/dialog/AppConfirm'
import { AppToast } from './components/toast/AppToast'
import { EmlExportProgress } from './components/toast/EmlExportProgress'
import { EnableEncryptionDialog } from './components/crypto/EnableEncryptionDialog'
import { VerifyKeyDialog } from './components/crypto/VerifyKeyDialog'
import { KeyLookupDialog } from './components/crypto/KeyLookupDialog'
import { CertificateTrustDialog } from './components/dialog/CertificateTrustDialog'
import { configurationRefresh } from './lib/configurationRefresh'
import { CONVERSATION_PAGE_SIZE } from './lib/pagination'
import { TagDialog } from './components/dialog/TagDialog'
import { invoke, invokeRetryingTimeout } from './lib/bridge'
import i18n, { resolveI18nLanguageFromWebLocale, useTranslation } from './lib/i18n'
import { isMac, type Chord } from './lib/shortcuts'
import { closeTopEscapeLayer } from './lib/useEscapeKey'
import { accounts$ } from './states/accounts'
import { closeComposeTabKeepingDraft, closeMessageTab } from './states/compose'
import { COMPOSE_TABS_KEY, TAB_VIEW_KEY, compose$ } from './states/composeState'
import { getComposeSession, closeComposeSessionKeepingDraft } from './states/composeSessions'
import { loadThread, mail$, normalizeMessageId } from './states/mail'
import { ensureThreadData } from './states/threadData'
import { hydrateSettings, SETTINGS_DB_KEYS, settings$, watchSystemAppearance } from './states/settings'
import { openThreadSearch } from './states/thread'
import { restoreThreadView, type ThreadViewSnapshot } from './states/threadView'
import { modalOpen, ui$ } from './states/ui'
import type { Account, Message, MessageTab } from './types'

type ThreadSession = {
  thread_id: string
  thread?: Message
  view?: ThreadViewSnapshot
  compose_tab?: MessageTab
  compose_activated?: boolean
  compose_revision?: number
}

// A reader boots only its account/settings and conversation. Main-window boot
// owns navigation persistence, background sync, notifications and media pruning.
export function ThreadWindow({ session }: { session: string }) {
  const { t } = useTranslation()
  const [ready, setReady] = useState(false)
  const [composerOnly, setComposerOnly] = useState(false)
  const [editorTabId, setEditorTabId] = useState('')
  const transferredTab = useRef<MessageTab | null>(null)
  const activationRequested = useRef(false)
  const composerRevision = useRef(0)
  const [isClosing, setIsClosing] = useState(false)
  const [error, setError] = useState('')
  const [attempt, setAttempt] = useState(0)
  const startup = useRef<Promise<void> | null>(null)
  const closing = useRef(false)
  const pendingRestore = useRef(false)
  const restoreMovedView = useCallback(() => {
    // Unmount the old reader before restoring its replacement snapshot so
    // scroll cleanup cannot overwrite it. Editor tabs remain in compose state.
    startup.current = null
    compose$.activeTab.set('')
    setReady(false)
    setError('')
    setAttempt((value) => value + 1)
  }, [])
  const editorExists = useValue(() => !!editorTabId && compose$.tabs.get().some((tab) => tab.id === editorTabId))
  const language = useValue(settings$.language)
  const tagThreadId = useValue(ui$.tagThreadId)
  const subject = useValue(() =>
    editorTabId
      ? compose$.tabs.get().find((tab) => tab.id === editorTabId)?.compose?.subject
      : mail$.threads.get().find((thread) => thread.thread_id === ui$.selectedThread.get())?.subject,
  )

  useEffect(() => {
    if (editorTabId && !editorExists) {
      void invoke('threadWindow.close', { session })
        .then(() => {
          localStorage.removeItem(COMPOSE_TABS_KEY)
          localStorage.removeItem(TAB_VIEW_KEY)
        })
        .catch(console.error)
    }
  }, [session, editorTabId, editorExists])

  useEffect(() => {
    void i18n.changeLanguage(language || resolveI18nLanguageFromWebLocale(navigator.language) || 'en')
  }, [language])
  useEffect(() => {
    void invoke('threadWindow.setReady', { session, ready, error: ready ? '' : error }).catch(console.error)
  }, [session, ready, error])

  useEffect(() => {
    document.title = `${subject || 'Meron'}${subject ? ' — Meron' : ''}`
  }, [subject])

  useEffect(() => {
    let disposed = false
    const refreshConfiguration = configurationRefresh()
    const activateComposer = () => {
      // An old renderer may have reported ready before this renderer reloads.
      // Remember activation even while its account/settings startup is pending.
      activationRequested.current = true
      const tab = transferredTab.current
      if (!tab) return
      transferredTab.current = null
      compose$.tabs.set([tab])
      compose$.activeTab.set(tab.id)
      setEditorTabId(tab.id)
    }
    startup.current ??= (async () => {
      const data = await invoke<ThreadSession>('threadWindow.get', { session })
      // Do not cancel the shared StrictMode startup on its first cleanup.
      const [accounts, { prefs }, result] = await Promise.all([
        invokeRetryingTimeout<{ accounts: Account[] }>('account.list'),
        invokeRetryingTimeout<{ prefs: Record<string, unknown> }>('app.prefsGet', { keys: SETTINGS_DB_KEYS }),
        data.compose_tab
          ? Promise.resolve<{ messages: Message[]; next_cursor?: string }>({ messages: [] })
          : invokeRetryingTimeout<{ messages: Message[]; next_cursor?: string }>('mail.threadRead', {
              thread_id: data.thread_id,
              limit: CONVERSATION_PAGE_SIZE,
            }),
      ])
      hydrateSettings(prefs)
      accounts$.set(accounts.accounts)
      if (data.compose_tab) {
        // The native snapshot is kept current in memory, including sensitive
        // drafts and attachment bytes that cannot go in localStorage.
        transferredTab.current = data.compose_tab
        composerRevision.current = data.compose_revision ?? 0
        setComposerOnly(true)
        if (data.compose_activated || activationRequested.current) activateComposer()
        return
      }
      const thread = result.messages.length ? (data.thread ?? result.messages[0]) : undefined
      if (!thread) throw new Error(t('compose.toast.couldNotOpenNotificationThread'))
      let messages = result.messages
      let cursor = result.next_cursor ?? ''
      const anchor = data.view?.scroll?.messageId
      const visited = new Set<string>()
      // Fetch the saved anchor before mounting: scroll restoration is consumed
      // on the first layout, so loading its older page afterwards is too late.
      while (anchor && !messages.some((message) => message.id === anchor) && cursor && !visited.has(cursor)) {
        visited.add(cursor)
        const page = await invokeRetryingTimeout<{ messages: Message[]; next_cursor?: string }>('mail.threadRead', {
          thread_id: data.thread_id,
          limit: CONVERSATION_PAGE_SIZE,
          before_cursor: cursor,
        })
        messages = [...page.messages, ...messages]
        cursor = page.next_cursor ?? ''
      }
      mail$.threads.set([thread])
      ensureThreadData(data.thread_id).assign({ messages, cursor })
      if (data.view) restoreThreadView(data.thread_id, data.view)
      ui$.selectedAccount.set(thread.account_id)
      ui$.selectedFolder.set(thread.folder_id)
      ui$.selectedThread.set(data.thread_id)
      ui$.mobilePane.set('conversation')
      mail$.messages.set(messages)
    })().catch((reason) => {
      startup.current = null
      throw reason
    })
    void startup.current
      .then(() => {
        if (!disposed) {
          setReady(true)
          setError('')
        }
      })
      .catch((reason) => {
        if (!disposed) setError(String(reason))
      })
    const refreshMail = () => {
      const id = ui$.selectedThread.peek()
      if (id)
        void loadThread(id)
          .then(() => {
            if (disposed || ui$.selectedThread.peek() !== id || mail$.threadErrorId.peek() === id) return
            const messages = mail$.messages.peek().filter((message) => message.thread_id === id)
            // The auxiliary window has no mailbox list to refresh its header row.
            // Message flags still follow the canonical page after another window acts.
            if (messages.length)
              mail$.threads.set(
                mail$.threads.peek().map((thread) =>
                  thread.thread_id === id
                    ? {
                        ...thread,
                        starred:
                          messages.some((message) => message.starred) ||
                          (!!ensureThreadData(id).cursor.peek() && thread.starred),
                      }
                    : thread,
                ),
              )
          })
          .catch(console.error)
    }
    let timer: ReturnType<typeof setTimeout>
    const scheduleRefresh = () => {
      clearTimeout(timer)
      timer = setTimeout(refreshMail, 100)
    }
    const eventsOn = (window as any).runtime?.EventsOn
    const stopActivate = eventsOn?.('threadWindow.activateComposer', (detail?: { session?: string }) => {
      if (detail?.session === session) activateComposer()
    })
    const stopRestoreView = eventsOn?.('threadWindow.restoreView', (detail?: { session?: string }) => {
      if (detail?.session !== session) return
      if (closing.current) pendingRestore.current = true
      else restoreMovedView()
    })
    const stops = ['mail.newMessages', 'mail.synced', 'mail.sentCopyCached'].map((name) =>
      eventsOn?.(name, scheduleRefresh),
    )
    stops.push(
      eventsOn?.('mail.windowChanged', (detail?: { thread_id?: string; starred?: boolean; message_ids?: string[] }) => {
        // A thread-wide star action covers history outside the loaded page too.
        if (detail?.thread_id && typeof detail.starred === 'boolean' && !detail.message_ids?.length) {
          mail$.threads.set(
            mail$.threads
              .peek()
              .map((thread) =>
                thread.thread_id === detail.thread_id ? { ...thread, starred: detail.starred! } : thread,
              ),
          )
        }
        scheduleRefresh()
      }),
    )
    stops.push(
      eventsOn?.('configuration.changed', (detail?: { keys?: string[] }) => {
        void refreshConfiguration.refresh(detail).catch(console.error)
      }),
    )
    const stopAppearance = watchSystemAppearance()
    // This window has one root conversation, including while an editor tab is
    // in front. Removing it must also retire the remembered Current tab.
    const stopSelection = ui$.selectedThread.onChange(({ value }) => compose$.conversationThread.set(value))
    window.addEventListener('focus', scheduleRefresh)
    return () => {
      disposed = true
      clearTimeout(timer)
      stopAppearance()
      stopSelection()
      stopRestoreView?.()
      stopActivate?.()
      refreshConfiguration.dispose()
      stops.forEach((stop) => stop?.())
      window.removeEventListener('focus', scheduleRefresh)
    }
  }, [session, attempt, restoreMovedView])

  useEffect(() => {
    if (!editorTabId) return
    let lastFiles = new Map<string, string>()
    let pending: MessageTab | undefined
    let publishing = false
    let stopped = false
    let retry: ReturnType<typeof setTimeout> | undefined
    // Coalesce edits while a write is in flight. Attachment bytes only cross
    // the bridge when added or changed, never on every keystroke.
    const drain = async () => {
      if (publishing || stopped) return
      clearTimeout(retry)
      retry = undefined
      publishing = true
      try {
        while (pending && !stopped) {
          const tab = pending
          pending = undefined
          const draft = tab.compose!
          const files = new Map(draft.attachments.map((file) => [file.id, file.data]))
          const data = Object.fromEntries(
            draft.attachments
              .filter((file) => lastFiles.get(file.id) !== file.data)
              .map((file) => [file.id, file.data]),
          )
          const result = await invoke<{ applied: boolean; revision: number }>('threadWindow.updateComposer', {
            session,
            revision: ++composerRevision.current,
            tab: {
              ...tab,
              compose: { ...draft, attachments: draft.attachments.map(({ data: _data, ...file }) => file) },
            },
            attachment_data: data,
          })
          composerRevision.current = Math.max(composerRevision.current, result.revision)
          if (result.applied) lastFiles = files
          else {
            // A write from the previous webview won this revision. Resend the
            // complete current snapshot with a fresh revision and all bytes.
            lastFiles.clear()
            pending ??= tab
          }
        }
      } catch (error) {
        console.error(error)
        lastFiles.clear()
        pending ??= compose$.tabs.peek().find((tab) => tab.id === editorTabId)
        if (!stopped && pending)
          retry = setTimeout(() => {
            retry = undefined
            void drain()
          }, 1000)
      } finally {
        publishing = false
        if (pending && !stopped && !retry) void drain()
      }
    }
    const publish = () => {
      const tab = compose$.tabs.peek().find((tab) => tab.id === editorTabId)
      if (!tab?.compose) return
      pending = tab
      void drain()
    }
    publish()
    const stop = compose$.tabs.onChange(publish)
    return () => {
      stopped = true
      clearTimeout(retry)
      stop()
    }
  }, [session, editorTabId])

  useEffect(() => {
    // Publish synchronously on changes, rather than waiting for a React render.
    // The native registry lets the main window focus this editor for a draft.
    let published = ''
    const publish = () => {
      const ids = compose$.tabs.peek().flatMap((tab) => (tab.compose ? [tab.compose.draftMessageId] : []))
      const next = JSON.stringify(ids)
      if (published === next) return
      published = next
      void invoke('threadWindow.setDrafts', { session, draft_ids: ids }).catch(console.error)
    }
    publish()
    const stopTabs = compose$.tabs.onChange(publish)
    const stopFocus = (window as any).runtime?.EventsOn?.(
      'threadWindow.focusDraft',
      (detail: { session?: string; draft_id?: string; previous_id?: string }) => {
        if (detail?.session !== session) return
        const tab = compose$.tabs
          .peek()
          .find(
            (tab) =>
              tab.compose &&
              [detail.draft_id, detail.previous_id]
                .filter(Boolean)
                .some((id) => normalizeMessageId(id!) === normalizeMessageId(tab.compose!.draftMessageId)),
          )
        if (tab) compose$.activeTab.set(tab.id)
      },
    )
    return () => {
      stopTabs()
      stopFocus?.()
    }
  }, [session])

  useEffect(() => {
    const close = async () => {
      if (closing.current) return
      closing.current = true
      setIsClosing(true)
      let closed = false
      try {
        await invoke('threadWindow.beginClose', { session })
        // Await any tab already saving/discarding, then save every remaining
        // editor (including hidden ones) before destroying this webview.
        for (const tab of compose$.tabs.peek().filter((tab) => tab.compose)) {
          const inProgress = getComposeSession(tab.id).closing
          if (inProgress) await inProgress
          if (!compose$.tabs.peek().some((item) => item.id === tab.id)) continue
          const saved = closeComposeTabKeepingDraft(tab.id, () => closeMessageTab(tab.id))
          if (!saved) return
          await saved
          if (compose$.tabs.peek().some((item) => item.id === tab.id)) return
        }
        if (compose$.tabs.peek().some((tab) => tab.compose)) return
        await invoke('threadWindow.close', { session })
        closed = true
        localStorage.removeItem(COMPOSE_TABS_KEY)
        localStorage.removeItem(TAB_VIEW_KEY)
      } catch (reason) {
        setError(String(reason))
      } finally {
        if (!closed) await invoke('threadWindow.cancelClose', { session }).catch(console.error)
        closing.current = false
        setIsClosing(false)
        if (!closed && pendingRestore.current) {
          pendingRestore.current = false
          restoreMovedView()
        }
      }
    }
    const stop = (window as any).runtime?.EventsOn?.(
      'threadWindow.closeRequested',
      (detail: { session?: string; request?: number }) => {
        if (detail?.session !== session) return
        void invoke('threadWindow.closeResponded', { session, request: detail.request }).catch(console.error)
        void close()
      },
    )
    const handleKey = (event: Event, chord: Chord) => {
      const command = chord.mod
      if (command && !chord.alt && !chord.shift && chord.key.toLowerCase() === 'w') {
        event.preventDefault()
        void close()
      } else if (command && !chord.alt && chord.key.toLowerCase() === 'f' && !modalOpen()) {
        event.preventDefault()
        openThreadSearch()
      } else if (chord.key === 'Escape') {
        if (closeTopEscapeLayer()) {
          event.preventDefault()
          return
        }
        if (!modalOpen() && closeComposeSessionKeepingDraft(compose$.activeTab.peek())) event.preventDefault()
      }
    }
    const keydown = (event: KeyboardEvent) =>
      handleKey(event, {
        key: event.key,
        mod: isMac ? event.metaKey : event.ctrlKey,
        alt: event.altKey,
        shift: event.shiftKey,
      })
    const frameKeydown = (event: Event) => {
      const chord = (event as CustomEvent<Chord>).detail
      if (chord?.key) handleKey(event, chord)
    }
    window.addEventListener('keydown', keydown)
    window.addEventListener('meron.frameKeyDown', frameKeydown)
    return () => {
      stop?.()
      window.removeEventListener('keydown', keydown)
      window.removeEventListener('meron.frameKeyDown', frameKeydown)
    }
  }, [session, restoreMovedView])

  return (
    <main aria-busy={isClosing} className="relative flex h-full flex-col bg-chat text-primary">
      {error && (
        <div role="alert" className="p-5 text-sm text-red-500">
          {error}
          {!ready && (
            <button className="ml-3 underline cursor-pointer" onClick={() => setAttempt((value) => value + 1)}>
              {t('chat.retry')}
            </button>
          )}
        </div>
      )}
      {!ready && !error && (
        <div role="status" className="p-5 text-sm text-secondary">
          {t('common.loading')}
        </div>
      )}
      {isClosing && (
        <div role="status" className="p-3 text-sm text-secondary">
          {t('composer.status.savingDraft')}
        </div>
      )}
      {ready && (
        <div inert={isClosing} className="relative flex min-h-0 flex-1 flex-col">
          {composerOnly ? editorTabId && <Composer tabId={editorTabId} /> : <MessagePane standalone />}
        </div>
      )}
      <AppConfirm />
      <AppToast />
      <EmlExportProgress />
      <EnableEncryptionDialog />
      <VerifyKeyDialog />
      <KeyLookupDialog />
      <CertificateTrustDialog />
      {tagThreadId && <TagDialog threadId={tagThreadId} />}
    </main>
  )
}
