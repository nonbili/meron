import { useEffect, useRef } from 'react'
import { useValue } from '@legendapp/state/react'
import { boot } from './boot'
import { invoke } from './lib/bridge'
import { ui$, showToast } from './states/ui'
import {
  mail$,
  loadFolders,
  loadThreads,
  loadThread,
  findLocalThread,
  refreshAccountFoldersCache,
  inboxUnread,
} from './states/mail'
import { openMailtoCompose, openThreadTabById } from './states/compose'
import { accounts$ } from './states/accounts'
import { kanban$ } from './states/kanban'
import { setSyncError, clearSyncErrorFor } from './states/connectivity'
import { settings$, applyDocumentLanguage } from './states/settings'
import { applyUpdateStatus, loadUpdateStatus, runUpdateCheck } from './states/update'
import type { UpdateStatus } from './lib/update'
import { useFoldersByAccount } from './lib/kanbanData'
import { setTrayUnread } from './lib/trayUnread'
import i18n, { resolveI18nLanguageFromWebLocale, t, translationTemplate } from './lib/i18n'
import { isConnectivitySyncError } from './components/banner/connectivityBannerHelpers'

const SEARCH_DEBOUNCE_MS = 300
const DEFAULT_RSS_SYNC_INTERVAL_MINUTES = 60
const MIN_RSS_SYNC_INTERVAL_MINUTES = 5
const MAX_RSS_SYNC_INTERVAL_MINUTES = 1440
// Long enough that a new release doesn't interrupt the first minute of use, and
// the boot sync has the network to itself.
const UPDATE_FIRST_CHECK_DELAY_MS = 30_000
const UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000

// All of App's side effects: startup boot/sync, per-selection read-state resets,
// RSS periodic sync, native event wiring (mailto, notifications, new mail/sync),
// mailbox/thread loading, and the tray unread badge. Kept out of the component so
// App stays a layout shell.
export function useAppEffects() {
  const accounts = useValue(accounts$)
  const foldersByAccount = useFoldersByAccount()
  const selectedAccount = useValue(ui$.selectedAccount)
  const selectedFolder = useValue(ui$.selectedFolder)
  const selectedThread = useValue(ui$.selectedThread)
  const query = useValue(ui$.query)
  const filterMode = useValue(ui$.filterMode)
  const activeBoardId = useValue(kanban$.activeBoardId)
  const startupSyncDone = useRef(false)
  const language = useValue(settings$.language)
  const showUnreadBadge = useValue(settings$.showUnreadAccountBadge)
  const autoUpdateCheck = useValue(settings$.autoUpdateCheck)

  useEffect(() => {
    const systemLanguage = resolveI18nLanguageFromWebLocale(navigator.language) || 'en'
    const targetLanguage = language || systemLanguage
    if (i18n.language !== targetLanguage) {
      void i18n.changeLanguage(targetLanguage)
    }
    void invoke('i18n.setNativeLabels', {
      trayShow: t('tray.showMeron'),
      trayHide: t('tray.hideToTray'),
      trayHideTooltip: t('tray.hideMeronTooltip'),
      trayQuit: t('tray.quitMeron'),
      newMessage: t('notify.newMessage'),
      newMessageCount: translationTemplate('notify.newMessageCount'),
      noSubject: t('notify.noSubject'),
      unknownSender: t('notify.unknownSender'),
    }).catch(() => {})
    // Reflect the resolved locale to <html lang>/dir, driving :lang() CJK glyph
    // selection (see index.css) and RTL. Synced to a paint-time cache in settings.
    applyDocumentLanguage(targetLanguage)
  }, [language])

  useEffect(() => {
    void boot().catch((error) => {
      const message = error instanceof Error ? error.message : String(error)
      console.error('App startup failed:', error)
      showToast(message, 'error')
    })
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
      void refreshAccountFoldersCache(account.id, false)
    }
  }, [showUnreadBadge, accounts])

  useEffect(() => {
    // Safety net: event-driven refreshes (refreshFoldersAfterFlagChange,
    // mail.synced/newMessages handlers below) can each individually miss an
    // account if their one-shot refreshAccountFoldersCache call fails silently.
    // Periodically re-seed every account's folder cache so a missed refresh
    // self-heals within one interval instead of leaving a badge stuck stale.
    if (!showUnreadBadge) return
    const timer = window.setInterval(() => {
      for (const account of accounts) {
        void refreshAccountFoldersCache(account.id, false)
      }
    }, 60_000)
    return () => window.clearInterval(timer)
  }, [showUnreadBadge, accounts])

  useEffect(() => {
    // Folders created outside Meron (webmail, another client) only reach the store
    // through a real LIST sync, and every other refresh here is cache-only. Without
    // this, a server-side folder stays invisible until the selected account changes
    // or the app restarts. Cheap: one LIST on a pooled session, deduped in core.
    if (accounts.length === 0) return
    const listFolders = () => {
      for (const account of accounts) {
        void refreshAccountFoldersCache(account.id, true)
      }
    }
    listFolders()
    const timer = window.setInterval(listFolders, 15 * 60_000)
    return () => window.clearInterval(timer)
  }, [accounts])

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

  // The core pushes an `error` event when it hits a condition it cannot serve
  // through — an unreachable keychain, an unopenable store. Nothing consumed it
  // before, so such a startup left the UI with no explanation and every request
  // failing on its own timeout.
  useEffect(() => {
    const eventsOn = (window as any).runtime?.EventsOn
    if (!eventsOn) return
    const offError = eventsOn('core.fatal', (detail: { message?: string } | string) => {
      const message = typeof detail === 'string' ? detail : detail?.message
      if (!message) return
      console.error('Mail engine error:', message)
      showToast(message, 'error')
    })
    return () => {
      if (typeof offError === 'function') offError()
    }
  }, [])

  // The updater's state machine lives in Go and pushes its whole status on every
  // transition, including download progress.
  useEffect(() => {
    void loadUpdateStatus()
    const eventsOn = (window as any).runtime?.EventsOn
    if (!eventsOn) return
    const offUpdate = eventsOn('update.status', (status: UpdateStatus) => {
      applyUpdateStatus(status)
    })
    return () => {
      if (typeof offUpdate === 'function') offUpdate()
    }
  }, [])

  // Background release polling. Finding an update only surfaces a banner; the
  // download never starts without the user asking for it.
  useEffect(() => {
    if (!autoUpdateCheck) return
    const first = window.setTimeout(() => void runUpdateCheck(), UPDATE_FIRST_CHECK_DELAY_MS)
    const repeat = window.setInterval(() => void runUpdateCheck(), UPDATE_CHECK_INTERVAL_MS)
    return () => {
      window.clearTimeout(first)
      window.clearInterval(repeat)
    }
  }, [autoUpdateCheck])

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

  // Also keyed on the open board: loads are skipped while one is up (the mail
  // list is off screen and its rows wait for the board to close), so closing it
  // has to reload — otherwise a board visit that never touched a card leaves the
  // selection unchanged, and the list stays as stale as the visit was long.
  useEffect(() => {
    if (!selectedAccount || !selectedFolder || activeBoardId) return
    if (!query.trim()) {
      void loadThreads()
      return
    }
    const timer = window.setTimeout(() => {
      void loadThreads()
    }, SEARCH_DEBOUNCE_MS)
    return () => window.clearTimeout(timer)
  }, [selectedAccount, selectedFolder, query, filterMode, activeBoardId])

  useEffect(() => {
    if (!selectedThread) return
    void loadThread(selectedThread)
  }, [selectedThread])

  useEffect(() => {
    // The tray mirrors INBOX unread only — that's the mail that raises new-mail
    // notifications. Other folders/labels (e.g. a "Notification" folder) carry
    // their own unread counts but must not keep the tray badge lit after the
    // inbox is read. Use the exact per-account cache that renders the sidebar;
    // launching separate folder queries here allowed a slow, stale result to
    // overwrite a newer unread event and leave the two indicators disagreeing.
    const unread = accounts.some((account) => inboxUnread(foldersByAccount[account.id]) > 0)
    setTrayUnread(unread)
  }, [accounts, foldersByAccount])

  useEffect(() => {
    const eventsOn = (window as any).runtime?.EventsOn
    if (!eventsOn) return

    const refreshCurrentMailbox = async () => {
      await loadThreads(false)
    }

    // The open conversation pane shows whichever thread is selected, which may
    // belong to a different account than the mailbox view — in a Kanban board or
    // the Starred view it's independent of `selectedAccount`. So reload it on its
    // own account's events, separately from the mailbox-list refresh below (which
    // is rightly scoped to the selected account). Without this, new mail in the
    // open thread updates the badge but never appears in the conversation.
    const refreshOpenThread = (eventAccount?: string) => {
      const openThread = ui$.selectedThread.get()
      if (!openThread) return
      if (eventAccount) {
        const threadAccount = findLocalThread(openThread)?.account_id
        if (threadAccount && threadAccount !== eventAccount) return
      }
      void loadThread(openThread).catch(console.error)
    }

    // Mail sync/folder fetch failed (network down, bad creds, timeout) — surface a
    // persistent banner that clears on the next good sync. Scoped to mail only;
    // RSS/store errors use the generic `error` event and don't raise this banner.
    const offError = eventsOn(
      'mail.syncError',
      (detail: { account?: string; message?: string; outer_timeout?: boolean }) => {
        const message = detail?.message ?? 'sync failed'
        if (isConnectivitySyncError(detail?.outer_timeout)) setSyncError(detail?.account ?? null, message)
      },
    )

    const offNew = eventsOn('mail.newMessages', (detail: { account?: string; folder?: string; count?: number }) => {
      // A successful fetch proves connectivity is back for this account.
      clearSyncErrorFor(detail?.account ?? null)
      // New mail arrived somewhere, so the tray should reflect unread immediately —
      // independent of which account/folder is selected. Clearing back to "read" is
      // handled by the reactive tray effect once the folder cache refreshes.
      setTrayUnread(true)
      // Keep the side navigation's per-account (and unified) unread badges honest for
      // *every* account, not just the selected one. get_folders recomputes unread
      // live, so this cache-only refresh picks up the new mail even when the
      // account is only visible as a Kanban column. Without it the badge stays
      // dark while the column (which falls back to counting loaded cards) shows
      // the real count. Done before the selection early-return below.
      if (detail?.account) void refreshAccountFoldersCache(detail.account, false)
      refreshOpenThread(detail?.account)
      if (detail?.account && selectedAccount !== 'unified' && detail.account !== selectedAccount) return
      const folder = detail?.folder ?? 'inbox'
      const count = detail?.count ?? 1
      showToast(`New mail in ${folder} (+${count})`)
      if (selectedAccount) void refreshCurrentMailbox().catch(console.error)
    })

    const offSynced = eventsOn('mail.synced', (detail: { account?: string; folders?: boolean }) => {
      clearSyncErrorFor(detail?.account ?? null)
      // A message-only sync (no folders:true) still changes the true unread count,
      // and get_folders recomputes it live — so refresh the synced account's folder
      // cache regardless, keeping the side navigation's per-account/unified badges in
      // sync for background accounts (e.g. ones only shown as a Kanban column).
      if (detail?.account) void refreshAccountFoldersCache(detail.account, false)
      if (detail?.folders) {
        if (!detail.account || selectedAccount === 'unified' || detail.account === selectedAccount) {
          void loadFolders(selectedAccount, false)
        }
      }
      refreshOpenThread(detail?.account)
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
