import type { Folder } from '../types'
import { invoke } from '../lib/bridge'
import { t } from '../lib/i18n'
import { confirmAction, ui$, showToast } from './ui'
import { unifiedAccounts } from './accounts'
import { forgetDeletedMailViewFolder, removeKanbanColumnsForFolder } from './kanban'
import { unifiedFolders } from '../lib/unifiedFolders'
import { findLocalThread, mail$ } from './mail'

// Folder lists, unread counts, and folder-level operations (empty, delete).

export function folderMatches(folder: Folder, accountId: string | undefined, folderId: string | undefined): boolean {
  if (!folderId) return false
  const wanted = folderId.toLowerCase()
  const folderIsInbox = folder.role === 'inbox' || folder.id.toLowerCase() === 'inbox'
  const idMatches = wanted === 'inbox' ? folderIsInbox : folder.id === folderId
  return idMatches && (!accountId || folder.account_id === accountId || folder.account_id === 'unified')
}

// Only refreshes are held. Mutation results and rollbacks must remain visible.
type FolderCountObservation = { unread: number; version: number }
type FolderReadHold = { pending: number; confirmed?: FolderCountObservation; mutation?: FolderCountObservation }
const pendingFolderReads = new Map<string, FolderReadHold>()
let folderReadVersion = 0
let oldestFolderReadVersion = 0
const folderReadVersions = new Map<string, number>()

function folderReadKey(accountId: string, folderId: string): string {
  return JSON.stringify([accountId, folderId.toLowerCase() === 'inbox' ? 'inbox' : folderId])
}

function touchFolderRead(key: string) {
  folderReadVersions.delete(key)
  folderReadVersions.set(key, ++folderReadVersion)
  // Old requests conservatively retain cached counts after history is evicted.
  if (folderReadVersions.size > 512) {
    const oldest = folderReadVersions.keys().next().value!
    oldestFolderReadVersion = folderReadVersions.get(oldest)!
    folderReadVersions.delete(oldest)
  }
}

// Capture before issuing the read, not when its response arrives.
export function captureFolderUnreadVersion(): number {
  return ++folderReadVersion
}

export function holdFolderUnread(
  accountId: string,
  folderId: string,
): (unread: number, confirmed: boolean, version?: number) => void {
  const key = folderReadKey(accountId, folderId)
  const hold = pendingFolderReads.get(key) ?? { pending: 0 }
  hold.pending++
  pendingFolderReads.set(key, hold)
  touchFolderRead(key)
  const started = folderReadVersion
  return (unread, confirmed, version = started) => {
    if (confirmed && (!hold.confirmed || version >= hold.confirmed.version)) hold.confirmed = { unread, version }
    const latest =
      hold.mutation && (!hold.confirmed || hold.mutation.version > hold.confirmed.version)
        ? hold.mutation
        : hold.confirmed
    updateCachedFolderUnread(accountId, folderId, latest?.unread ?? unread, 'local')
    if (--hold.pending === 0) pendingFolderReads.delete(key)
    touchFolderRead(key)
  }
}

function folderUnreadHeld(accountId: string, folder: Folder): boolean {
  return pendingFolderReads.has(folderReadKey(accountId, folder.id))
}

function reconcileFolderCounts(accountId: string, folders: Folder[], version: number): Folder[] {
  const current = mail$.foldersByAccount[accountId].get() ?? []
  return folders.map((folder) => {
    const key = folderReadKey(accountId, folder.id)
    const changed = version < oldestFolderReadVersion || (folderReadVersions.get(key) ?? 0) > version
    if (!changed && !folderUnreadHeld(accountId, folder)) return folder
    const cached = current.find((item) => folderReadKey(accountId, item.id) === key)
    return cached ? { ...folder, unread: cached.unread } : folder
  })
}

export function decrementFolderUnread(accountId: string | undefined, folderId: string | undefined, count: number) {
  if (count <= 0 || !folderId) return
  const dec = (folder: Folder) =>
    folderMatches(folder, accountId, folderId) ? { ...folder, unread: Math.max(0, folder.unread - count) } : folder

  mail$.folders.set(mail$.folders.get().map(dec))
  if (accountId) {
    const byAccount = mail$.foldersByAccount.get()
    const accountFolders = byAccount[accountId]
    if (accountFolders) {
      mail$.foldersByAccount.set({
        ...byAccount,
        [accountId]: accountFolders.map(dec),
      })
    }
  }
}

function looksLikeTrashName(value: string): boolean {
  return ['trash', 'bin', 'deleted items', 'deleted messages', '[gmail]/trash'].includes(value.trim().toLowerCase())
}

export function isTrashFolder(folder?: Pick<Folder, 'id' | 'name' | 'role'> | null): boolean {
  if (!folder) return false
  return folder.role === 'trash' || looksLikeTrashName(folder.id) || looksLikeTrashName(folder.name)
}

export function isTrashFolderId(accountId: string, folderId: string): boolean {
  const accountFolders = mail$.foldersByAccount[accountId].get() ?? []
  const selectedFolders = mail$.folders.get()
  const folder = [...accountFolders, ...selectedFolders].find(
    (item) => item.account_id === accountId && item.id === folderId,
  )
  return isTrashFolder(folder) || looksLikeTrashName(folderId)
}

function looksLikeJunkName(value: string): boolean {
  return ['junk', 'spam', 'junk e-mail', 'junk email', 'bulk mail', '[gmail]/spam'].includes(value.trim().toLowerCase())
}

// Trash and Junk are the only folders that may be emptied: the delete is
// permanent, so anywhere else it would be an unrecoverable mis-tap. Takes the
// folder the caller already has in hand (so the lookup stays inside the
// caller's reactive folder list) and returns the role plus display name for the
// menu label and confirm wording, or null when the folder is not emptiable.
export function emptiableFolder(folder?: Folder | null): { role: 'trash' | 'junk'; name: string } | null {
  if (!folder) return null
  const name = folder.name || folder.id
  if (folder.role === 'junk' || looksLikeJunkName(folder.id) || looksLikeJunkName(folder.name)) {
    return { role: 'junk', name }
  }
  if (isTrashFolder(folder)) return { role: 'trash', name }
  return null
}

// Permanently delete every message in a Trash or Junk folder, server side and in
// the store. Confirms first — there is no Trash left to restore from. `target`
// comes from `emptiableFolder`; the sidecar re-checks the folder role anyway.
// Returns true when the folder was emptied, so the caller can refresh its view.
export async function emptyFolder(
  accountId: string,
  folderId: string,
  target: { role: 'trash' | 'junk'; name: string },
): Promise<boolean> {
  if (!accountId || !folderId || accountId === 'unified') return false

  if (
    !(await confirmAction({
      title: t('threads.emptyFolder.confirmTitle', { folder: target.name }),
      message: t('threads.emptyFolder.confirmMessage', { folder: target.name }),
      confirmLabel: t('threads.emptyFolder.confirmButton'),
      tone: 'danger',
    }))
  ) {
    return false
  }

  try {
    const res = await invoke<MutationResult>('mail.emptyFolder', { account_id: accountId, folder_id: folderId })
    applyMutationFolderUnreads(res)
    // The open conversation may have just been deleted along with the folder.
    const openThread = findLocalThread(ui$.selectedThread.get())
    if (openThread?.account_id === accountId && openThread?.folder_id === folderId) ui$.selectedThread.set('')
    void loadFolders(accountId, false)
    showToast(t('threads.emptyFolder.done', { folder: target.name }))
    return true
  } catch (error) {
    showToast(
      error instanceof Error ? error.message : t('threads.emptyFolder.failed', { folder: target.name }),
      'error',
    )
    return false
  }
}

// Only an ordinary folder can be deleted on the server: special-use mailboxes
// carry the app's own routing (Inbox/Sent/Drafts/Trash/Junk/Archive), and that
// covers anything nested under the folder too, because deleting it takes the
// whole subtree along. Takes the account's folder list so the nesting check
// stays inside the caller's reactive list, and returns the display name for the
// menu label plus the number of subfolders that would go with it, or null when
// not deletable.
export function deletableFolder(
  folder: Folder | undefined | null,
  folders: Folder[],
): { name: string; nested: number } | null {
  if (!folder || !folder.id || folder.account_id === 'unified') return null
  if (folder.role && folder.role !== 'folder') return null
  const prefix = `${folder.id}${folder.delimiter || '/'}`
  const nested = folders.filter((item) => item.id !== folder.id && item.id.startsWith(prefix))
  if (nested.some((item) => item.role && item.role !== 'folder')) return null
  return { name: folder.name || folder.id, nested: nested.length }
}

// Delete a folder on the server, along with its subfolders, their cached
// messages and any board column that showed one of them. Confirms first — the
// mail goes with them and the server keeps no copy. Core re-checks that the
// subtree is deletable. Returns true when the folder is gone, so the caller can
// move its view elsewhere.
export async function deleteFolder(accountId: string, folderId: string, name?: string, nested = 0): Promise<boolean> {
  if (!accountId || !folderId || accountId === 'unified') return false
  const label = name || folderId

  if (
    !(await confirmAction({
      title: t('folders.delete.confirmTitle', { folder: label }),
      message: nested
        ? t('folders.delete.confirmMessageNested', { folder: label, count: nested })
        : t('folders.delete.confirmMessage', { folder: label }),
      confirmLabel: t('folders.delete.confirmButton'),
      tone: 'danger',
    }))
  ) {
    return false
  }

  try {
    const res = await invoke<{ folders?: Folder[]; removed?: string[]; warning?: string }>('mail.folderDelete', {
      account_id: accountId,
      folder_id: folderId,
    })
    // Core reports the whole subtree it took down; fall back to the folder
    // itself if an older core answers without the list.
    const removed = res?.removed?.length ? res.removed : [folderId]
    const folders = res?.folders
    if (folders) {
      mail$.foldersByAccount[accountId].set(folders)
      if (ui$.selectedAccount.get() === accountId) mail$.folders.set(folders)
    } else {
      void loadFolders(accountId, false)
    }
    // Capture this before removing the Kanban column, which also drops the
    // column's thread cache and would make the selected conversation unfindable.
    const openThread = findLocalThread(ui$.selectedThread.get())
    for (const gone of removed) removeKanbanColumnsForFolder(accountId, gone)
    // The open conversation and the mailbox view may have been inside any of
    // the deleted folders, not just the one the action targeted.
    if (openThread?.account_id === accountId && removed.includes(openThread?.folder_id ?? '')) {
      ui$.selectedThread.set('')
    }
    if (ui$.selectedAccount.get() === accountId) {
      if (removed.includes(ui$.selectedFolder.get())) ui$.selectedFolder.set('inbox')
      // While a board is open the visible folder is the open card's, not the
      // mail view's — that one is stashed for the board's close, so it needs
      // the same check.
      forgetDeletedMailViewFolder(removed)
    }
    if (res?.warning) {
      showToast(res.warning, 'error')
      return false
    }
    showToast(t('folders.delete.done', { folder: label }))
    return true
  } catch (error) {
    showToast(error instanceof Error ? error.message : t('folders.delete.failed', { folder: label }), 'error')
    return false
  }
}

export async function loadFolders(accountId: string, refresh = true) {
  if (accountId === 'unified') {
    const accounts = unifiedAccounts()
    // Publish the synthetic list before the per-account counts land: it never
    // depends on them, and the folder switcher reads it from the per-account
    // cache — leaving that empty until the fan-out resolves shows the picker's
    // "no folders" state on every cold open.
    mail$.folders.set(unifiedFolders(t))
    mail$.foldersByAccount['unified'].set(unifiedFolders(t))
    let totalUnread = 0
    try {
      const foldersList = await Promise.all(
        accounts.map(async (acc) => {
          try {
            const version = folderReadVersion
            const res = await invoke<{ folders: Folder[] }>('mail.folderList', {
              account_id: acc.id,
              // Propagate the caller's refresh so sub-accounts get a real folder
              // LIST sync in the unified view. Without it a freshly added account
              // only ever has its synthetic INBOX row, so the folder picker and
              // "Move to" lists show just Inbox. The sync is async + deduped; the
              // mail.synced({folders:true}) it emits triggers a refresh:false reload.
              refresh,
            })
            const folders = reconcileFolderCounts(acc.id, res.folders || [], version)
            mail$.foldersByAccount[acc.id].set(folders)
            return folders
          } catch {
            return []
          }
        }),
      )
      for (const folders of foldersList) {
        const inboxFolder = folders.find((f) => f.role === 'inbox' || f.id.toLowerCase() === 'inbox')
        if (inboxFolder) {
          totalUnread += inboxFolder.unread || 0
        }
      }
    } catch (err) {
      console.error('Failed to load folders list for unified count:', err)
    }

    const folders = unifiedFolders(t, totalUnread)
    mail$.folders.set(folders)
    mail$.foldersByAccount['unified'].set(folders)
    return
  }

  const version = folderReadVersion
  const result = await invoke<{ folders: Folder[] }>('mail.folderList', { account_id: accountId, refresh })
  // A response without a folders array (an empty JSON object from the sidecar)
  // must not leave the stores holding `undefined`: every reader spreads them.
  const folders = reconcileFolderCounts(accountId, result.folders || [], version)
  mail$.folders.set(folders)
  mail$.foldersByAccount[accountId].set(folders)
}

export async function refreshAccountFoldersCache(accountId: string, refresh = false): Promise<Folder[]> {
  if (!accountId || accountId === 'unified') return []
  try {
    const version = folderReadVersion
    const result = await invoke<{ folders: Folder[] }>('mail.folderList', { account_id: accountId, refresh })
    const folders = reconcileFolderCounts(accountId, result.folders || [], version)
    mail$.foldersByAccount[accountId].set(folders)
    return result.folders || []
  } catch (error) {
    console.error('refreshAccountFoldersCache failed:', error)
    return []
  }
}

/**
 * Republish the active folder list from the per-account cache, but only while
 * `accountId` is still the selected account. For callers that refreshed the
 * cache with [refreshAccountFoldersCache] after an await and must not replace
 * the folders of a view the user has since switched to.
 */
export function publishCachedFolders(accountId: string) {
  if (!accountId || ui$.selectedAccount.peek() !== accountId) return
  const byAccount = mail$.foldersByAccount.get()
  if (accountId === 'unified') {
    const total = unifiedAccounts().reduce((sum, account) => sum + inboxUnread(byAccount[account.id]), 0)
    const folders = unifiedFolders(t, total)
    mail$.folders.set(folders)
    mail$.foldersByAccount['unified'].set(folders)
    return
  }
  const folders = byAccount[accountId]
  if (folders) mail$.folders.set(folders)
}

/** Unread count of the INBOX folder in a folder list, or 0 if absent. */
export function inboxUnread(folders: Folder[] | undefined): number {
  return folderUnread(folders, 'inbox')
}

/** Unread count of a folder in a folder list, treating INBOX case-insensitively. */
export function folderUnread(folders: Folder[] | undefined, folderId: string): number {
  if (!folders) return 0
  const wanted = folderId.toLowerCase()
  const folder = folders.find((f) => {
    if (wanted === 'inbox') return f.role === 'inbox' || f.id.toLowerCase() === 'inbox'
    return f.id === folderId
  })
  return folder?.unread ?? 0
}

// Apply an unread total returned with a thread page to the same per-account
// folder cache used by side-navigation badges. This keeps a freshly loaded
// mailbox/Kanban column and the navigation chrome on one core-owned value.
export function updateCachedFolderUnread(
  accountId: string,
  folderId: string,
  unread: number,
  source: 'refresh' | 'mutation' | 'local' = 'refresh',
) {
  if (!accountId || accountId === 'unified' || !folderId || !Number.isFinite(unread)) return
  const count = Math.max(0, Math.floor(unread))
  if (source === 'mutation') {
    const key = folderReadKey(accountId, folderId)
    const hold = pendingFolderReads.get(key)
    touchFolderRead(key)
    if (hold) hold.mutation = { unread: count, version: folderReadVersion }
  }
  const patch = (folders: Folder[] | undefined): Folder[] => {
    const current = folders ?? []
    let matched = false
    const next = current.map((folder) => {
      if (!folderMatches(folder, accountId, folderId)) return folder
      matched = true
      if (source === 'refresh' && folderUnreadHeld(accountId, folder)) return folder
      return folder.unread === count ? folder : { ...folder, unread: count }
    })
    if (matched || folderId.toLowerCase() !== 'inbox') return next
    return [...next, { id: folderId, account_id: accountId, name: 'Inbox', role: 'inbox', unread: count }]
  }

  const byAccount = mail$.foldersByAccount.get()
  const nextAccountFolders = patch(byAccount[accountId])
  const nextByAccount = { ...byAccount, [accountId]: nextAccountFolders }
  mail$.foldersByAccount.set(nextByAccount)

  const selected = ui$.selectedAccount.get()
  if (selected === accountId) {
    mail$.folders.set(patch(mail$.folders.get()))
  } else if (selected === 'unified') {
    const total = unifiedAccounts().reduce((sum, account) => sum + inboxUnread(nextByAccount[account.id]), 0)
    mail$.folders.set(unifiedFolders(t, total))
  }
}

export type MutationResult = { folder_unreads?: Record<string, Record<string, number>> }

export function applyMutationFolderUnreads(result: MutationResult | undefined) {
  for (const [accountId, folders] of Object.entries(result?.folder_unreads ?? {})) {
    for (const [folderId, unread] of Object.entries(folders)) {
      updateCachedFolderUnread(accountId, folderId, unread, 'mutation')
    }
  }
}

function hasOnlyBootstrapInbox(folders: Folder[]) {
  if (folders.length !== 1) return false
  const folder = folders[0]
  return folder.role === 'inbox' || folder.id.toLowerCase() === 'inbox' || folder.name.toLowerCase() === 'inbox'
}

function sleep(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms))
}

// Folders for a specific account, fetching+caching them if not already loaded.
// Used by per-thread actions (e.g. the context menu's "Move to") that may run in
// the unified view, where `mail$.folders` only holds the synthetic unified inbox.
export async function ensureAccountFolders(
  accountId: string,
  options: { refreshIfBootstrapOnly?: boolean; waitForRefresh?: boolean; forceRefresh?: boolean } = {},
): Promise<Folder[]> {
  if (!accountId || accountId === 'unified') return []
  const cached = mail$.foldersByAccount[accountId].get()
  if (cached && cached.length > 0) {
    // A non-empty cache is not proof it is current: folders created on the server
    // (webmail, another client) only reach us through a real LIST sync. Callers
    // that show a folder picker pass forceRefresh so the list self-heals; the
    // result lands asynchronously via mail.synced({folders:true}).
    if (options.forceRefresh || (options.refreshIfBootstrapOnly && hasOnlyBootstrapInbox(cached))) {
      void refreshAccountFoldersCache(accountId, true)
    }
    return cached
  }
  try {
    const version = folderReadVersion
    const result = await invoke<{ folders: Folder[] }>('mail.folderList', {
      account_id: accountId,
      refresh: options.refreshIfBootstrapOnly || options.forceRefresh,
    })
    const folders = reconcileFolderCounts(accountId, result.folders || [], version)
    mail$.foldersByAccount[accountId].set(folders)
    if (options.waitForRefresh && (options.refreshIfBootstrapOnly || options.forceRefresh) && folders.length === 0) {
      for (let attempt = 0; attempt < 10; attempt += 1) {
        await sleep(500)
        const refreshed = await refreshAccountFoldersCache(accountId, false)
        if (refreshed.length > 0) return refreshed
      }
    }
    return folders
  } catch {
    return []
  }
}

// Prefer the core-provided folder role. The name fallback is only for call
// sites that have a bare folder id before folder metadata is loaded.
export function isDraftFolder(folderId: string, accountId?: string): boolean {
  const candidates = [...(accountId ? (mail$.foldersByAccount[accountId].get() ?? []) : []), ...mail$.folders.get()]
  const folder = candidates.find((item) => item.id === folderId || item.name === folderId)
  if (folder?.role) return folder.role === 'drafts'
  return ['drafts', 'draft', 'inbox.drafts', 'inbox.draft', '[gmail]/drafts', '[gmail]/draft'].includes(
    folderId.toLowerCase(),
  )
}

/** Whether a message lives in the account's inbox — i.e. it was delivered to us.
 * Same role-first, name-fallback shape as {@link isDraftFolder}. */
export function isInboxFolder(folderId: string, accountId?: string): boolean {
  const candidates = [...(accountId ? (mail$.foldersByAccount[accountId].get() ?? []) : []), ...mail$.folders.get()]
  const folder = candidates.find((item) => item.id === folderId || item.name === folderId)
  if (folder?.role) return folder.role === 'inbox'
  return folderId.toLowerCase() === 'inbox'
}
