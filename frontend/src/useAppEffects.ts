import { useEffect, useRef } from 'react'
import { useValue } from '@legendapp/state/react'
import { boot } from './boot'
import { invoke } from './lib/bridge'
import { ui$, showToast } from './states/ui'
import { mail$, loadFolders, loadThreads, loadThread, refreshAccountFoldersCache } from './states/mail'
import { openMailtoCompose, openThreadTabById } from './states/compose'
import { accounts$ } from './states/accounts'
import { closeKanbanBoard, kanban$ } from './states/kanban'
import { setSyncError, clearSyncErrorFor } from './states/connectivity'
import { settings$, applyDocumentLanguage } from './states/settings'
import i18n, { resolveI18nLanguageFromWebLocale } from './lib/i18n'

const SEARCH_DEBOUNCE_MS = 300
const DEFAULT_RSS_SYNC_INTERVAL_MINUTES = 60
const MIN_RSS_SYNC_INTERVAL_MINUTES = 5
const MAX_RSS_SYNC_INTERVAL_MINUTES = 1440

// All of App's side effects: startup boot/sync, per-selection read-state resets,
// RSS periodic sync, native event wiring (mailto, notifications, new mail/sync),
// mailbox/thread loading, and the tray unread badge. Kept out of the component so
// App stays a layout shell.
export function useAppEffects() {
  const accounts = useValue(accounts$)
  const folders = useValue(mail$.folders)
  const threads = useValue(mail$.threads)
  const selectedAccount = useValue(ui$.selectedAccount)
  const selectedFolder = useValue(ui$.selectedFolder)
  const selectedThread = useValue(ui$.selectedThread)
  const query = useValue(ui$.query)
  const filterMode = useValue(ui$.filterMode)
  const startupSyncDone = useRef(false)
  const language = useValue(settings$.language)
  const showUnreadBadge = useValue(settings$.showUnreadAccountBadge)

  useEffect(() => {
    const systemLanguage = resolveI18nLanguageFromWebLocale(navigator.language) || 'en'
    const targetLanguage = language || systemLanguage
    if (i18n.language !== targetLanguage) {
      void i18n.changeLanguage(targetLanguage)
    }
    // Reflect the resolved locale to <html lang>/dir, driving :lang() CJK glyph
    // selection (see index.css) and RTL. Synced to a paint-time cache in settings.
    applyDocumentLanguage(targetLanguage)
  }, [language])

  useEffect(() => {
    void boot()
  }, [])

  useEffect(() => {
    const unsubAccount = ui$.selectedAccount.onChange(() => mail$.readThreads.set({}))
    const unsubFolder = ui$.selectedFolder.onChange(() => mail$.readThreads.set({}))
    const unsubFilter = ui$.filterMode.onChange(() => mail$.readThreads.set({}))
    const unsubBoard = kanban$.activeBoardId.onChange(() => mail$.readThreads.set({}))
    const unsubGlobalFilter = kanban$.globalFilter.onChange(() => mail$.readThreads.set({}))
    return () => {
      unsubAccount()
      unsubFolder()
      unsubFilter()
      unsubBoard()
      unsubGlobalFilter()
    }
  }, [])

  useEffect(() => {
    // When the avatar unread-badge setting is on, seed every account's folder
    // cache so badges show for all accounts — not just the selected one (which
    // loadFolders covers). Cache-only (refresh:false), so no IMAP round-trip;
    // mail.synced keeps these fresh afterwards. Covers paused accounts too.
    if (!showUnreadBadge) return
    for (const account of accounts) {
      const isRSS = account.provider === 'rss' || account.auth_type === 'rss'
      if (isRSS) continue
      void refreshAccountFoldersCache(account.id, false)
    }
  }, [showUnreadBadge, accounts])

  useEffect(() => {
    if (startupSyncDone.current || accounts.length === 0) return
    startupSyncDone.current = true
    for (const account of accounts) {
      if (account.paused) continue
      void invoke('mail.sync', { account_id: account.id }).catch(() => {})
    }
  }, [accounts])

  useEffect(() => {
    const timers: number[] = []
    for (const account of accounts) {
      const isRSS = account.provider === 'rss' || account.auth_type === 'rss'
      if (!isRSS || account.paused) continue
      const minutes = Math.min(
        MAX_RSS_SYNC_INTERVAL_MINUTES,
        Math.max(MIN_RSS_SYNC_INTERVAL_MINUTES, account.rss_sync_interval_minutes ?? DEFAULT_RSS_SYNC_INTERVAL_MINUTES),
      )
      const timer = window.setInterval(() => {
        void invoke('mail.sync', { account_id: account.id }).catch(() => {})
      }, minutes * 60_000)
      timers.push(timer)
    }
    return () => {
      timers.forEach((timer) => window.clearInterval(timer))
    }
  }, [accounts])

  useEffect(() => {
    const eventsOn = (window as any).runtime?.EventsOn
    if (!eventsOn) return
    const offMailto = eventsOn('mailto.open', (raw: string) => {
      openMailtoCompose(raw)
    })
    return () => {
      if (typeof offMailto === 'function') offMailto()
    }
  }, [])

  useEffect(() => {
    const eventsOn = (window as any).runtime?.EventsOn
    if (!eventsOn) return
    const offNotification = eventsOn(
      'notification-clicked',
      (detail: { account?: string; threadId?: string; threadKey?: string }) => {
        const threadId = detail?.threadId || detail?.threadKey
        if (threadId) {
          void openThreadTabById(threadId)
        }
      },
    )
    return () => {
      if (typeof offNotification === 'function') offNotification()
    }
  }, [])

  useEffect(() => {
    if (!selectedAccount) return
    void loadFolders(selectedAccount)
  }, [selectedAccount])

  useEffect(() => {
    if (!selectedAccount || !selectedFolder) return
    if (!query.trim()) {
      void loadThreads()
      return
    }
    const timer = window.setTimeout(() => {
      void loadThreads()
    }, SEARCH_DEBOUNCE_MS)
    return () => window.clearTimeout(timer)
  }, [selectedAccount, selectedFolder, query, filterMode])

  useEffect(() => {
    if (!selectedThread) return
    void loadThread(selectedThread)
  }, [selectedThread])

  useEffect(() => {
    // The tray mirrors INBOX unread only — that's the mail that raises new-mail
    // notifications. Other folders/labels (e.g. a "Notification" folder) carry
    // their own unread counts but must not keep the tray badge lit after the
    // inbox is read.
    const localInboxUnread = folders.some((folder) => folder.role === 'inbox' && folder.unread > 0)

    if (localInboxUnread || accounts.length === 0) {
      void invoke('tray.setUnread', { unread: localInboxUnread }).catch(() => {})
      return
    }

    let cancelled = false
    void Promise.all(
      accounts.map((account) =>
        invoke<{ folders: { role: string; unread: number }[] }>('mail.folderList', {
          account_id: account.id,
          refresh: false,
        }).catch(() => ({ folders: [] })),
      ),
    ).then((results) => {
      if (cancelled) return
      const unread = results.some((result) =>
        result.folders.some((folder) => folder.role === 'inbox' && folder.unread > 0),
      )
      void invoke('tray.setUnread', { unread }).catch(() => {})
    })

    return () => {
      cancelled = true
    }
  }, [accounts, folders, threads])

  useEffect(() => {
    const eventsOn = (window as any).runtime?.EventsOn
    if (!eventsOn) return

    const refreshCurrentMailbox = async () => {
      const openThread = ui$.selectedThread.get()
      await loadThreads(false)
      const currentThread = ui$.selectedThread.get()
      if (currentThread && currentThread === openThread) {
        await loadThread(currentThread)
      }
    }

    // Mail sync/folder fetch failed (network down, bad creds, timeout) — surface a
    // persistent banner that clears on the next good sync. Scoped to mail only;
    // RSS/store errors use the generic `error` event and don't raise this banner.
    const offError = eventsOn('mail.syncError', (detail: { account?: string; message?: string }) => {
      setSyncError(detail?.account ?? null, detail?.message ?? 'sync failed')
    })

    const offNew = eventsOn('mail.newMessages', (detail: { account?: string; folder?: string; count?: number }) => {
      // A successful fetch proves connectivity is back for this account.
      clearSyncErrorFor(detail?.account ?? null)
      // New mail arrived somewhere, so the tray should reflect unread immediately —
      // independent of which account/folder is selected. Clearing back to "read" is
      // handled by the reactive tray effect once folders/threads refresh.
      void invoke('tray.setUnread', { unread: true }).catch(() => {})
      if (detail?.account && selectedAccount !== 'unified' && detail.account !== selectedAccount) return
      const folder = detail?.folder ?? 'inbox'
      const count = detail?.count ?? 1
      showToast(`New mail in ${folder} (+${count})`)
      if (selectedAccount) void refreshCurrentMailbox().catch(console.error)
    })

    const offSynced = eventsOn('mail.synced', (detail: { account?: string; folders?: boolean }) => {
      clearSyncErrorFor(detail?.account ?? null)
      if (detail?.folders) {
        if (detail.account) void refreshAccountFoldersCache(detail.account, false)
        if (!detail.account || selectedAccount === 'unified' || detail.account === selectedAccount) {
          void loadFolders(selectedAccount, false)
        }
      }
      if (detail?.account && selectedAccount !== 'unified' && detail.account !== selectedAccount) return
      void refreshCurrentMailbox().catch(console.error)
    })

    return () => {
      if (typeof offError === 'function') offError()
      if (typeof offNew === 'function') offNew()
      if (typeof offSynced === 'function') offSynced()
    }
  }, [selectedAccount, selectedFolder, query])
}
