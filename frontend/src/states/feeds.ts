import { invoke } from '../lib/bridge'
import { boot } from '../boot'
import { ui$, showToast } from './ui'
import { loadThreads, loadFolders } from './mail'

// RSS/feed management. RSS accounts hold feeds; each feed surfaces as a thread.
// These drive the add-feed / edit-feed dialogs and OPML import/export.
export const RSS_FEED_DRAG_TYPE = 'application/x-meron-rss-feed'

// Open the in-app "Add feed" dialog for an RSS account.
export function openAddFeed(accountId: string) {
  if (!accountId || accountId === 'unified') return
  ui$.addFeedAccount.set(accountId)
}

// Add a feed to an existing RSS account, then refresh the account's threads
// (each feed shows as a thread) and its folder unread count. Throws on failure
// so the dialog can surface the error inline.
export async function submitFeed(accountId: string, url: string) {
  await invoke('rss.addFeed', { account_id: accountId, feed_url: url.trim() })
  await Promise.all([loadThreads(false), loadFolders(accountId, false)])
}

// Open the feed edit modal for a single RSS feed (thread).
export function openFeedEdit(feed: { threadId: string; name: string; url?: string }) {
  if (!feed.threadId) return
  ui$.editFeed.set(feed)
}

// Remove a single feed (RSS subscription) by its thread id, then refresh the
// account's threads and unread count. The parent RSS account stays in place.
// Confirmation happens in the feed edit modal, so no native prompt here.
export async function removeFeed(threadId: string) {
  if (!threadId) return
  try {
    await invoke('rss.removeFeed', { thread_id: threadId })
    if (ui$.selectedThread.get() === threadId) {
      ui$.selectedThread.set('')
    }
    ui$.editFeed.set(null)
    showToast('Feed removed')
    // A feed removal is an explicit list mutation, so replace the current list.
    // Background loads merge missing first-page rows back in to preserve pages
    // loaded by scrolling, which would keep the removed feed visible.
    await Promise.all([loadThreads(true), loadFolders(ui$.selectedAccount.get(), false)])
  } catch (error) {
    showToast(error instanceof Error ? error.message : 'Failed to remove feed', 'error')
  }
}

function rssAccountFromThreadId(threadId: string) {
  const marker = '#rss#'
  const index = threadId.indexOf(marker)
  return index > 0 ? threadId.slice(0, index) : ''
}

// Move a feed subscription between RSS accounts while preserving its cached
// items and item read/starred state.
export async function moveFeed(threadId: string, targetAccountId: string) {
  if (!threadId || !targetAccountId) return
  const sourceAccountId = rssAccountFromThreadId(threadId)
  if (sourceAccountId && sourceAccountId === targetAccountId) return
  try {
    await invoke('rss.moveFeed', { thread_id: threadId, target_account_id: targetAccountId })
    if (ui$.selectedThread.get() === threadId) {
      ui$.selectedThread.set('')
    }
    ui$.editFeed.set(null)
    await Promise.all([
      loadThreads(false),
      sourceAccountId ? loadFolders(sourceAccountId, false) : Promise.resolve(),
      loadFolders(targetAccountId, false),
    ])
    showToast('Feed moved')
  } catch (error) {
    showToast(error instanceof Error ? error.message : 'Failed to move feed', 'error')
  }
}

// Export one RSS account's feeds to an OPML file the user picks via a native save
// dialog. The bridge gathers the feeds and writes the file; we surface the result.
export async function exportOpml(account: string) {
  try {
    const res = await invoke<{ saved: boolean; path?: string }>('rss.exportOpml', { account })
    if (res?.saved) showToast('Feeds exported')
  } catch (error) {
    showToast(error instanceof Error ? error.message : 'Export failed', 'error')
  }
}

// Import feeds from an OPML file the user picks via a native open dialog into the
// given RSS account. On a successful import, refresh that account's feeds and kick
// a sync so the new feeds populate.
export async function importOpml(account: string) {
  try {
    const res = await invoke<{ imported: number; cancelled?: boolean }>('rss.importOpml', { account })
    if (res?.cancelled) return
    const count = res?.imported ?? 0
    if (count === 0) {
      showToast('No new feeds to import')
      return
    }
    await boot()
    await Promise.all([loadThreads(false), loadFolders(ui$.selectedAccount.get(), false)])
    // Kick a background sync so the new feeds populate; the sidecar emits sync
    // events that refresh the UI as items arrive.
    void invoke('mail.sync', { account_id: account }).catch(() => {})
    showToast(`Imported ${count} ${count === 1 ? 'feed' : 'feeds'}`)
  } catch (error) {
    showToast(error instanceof Error ? error.message : 'Import failed', 'error')
  }
}
