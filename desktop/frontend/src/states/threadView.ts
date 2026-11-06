import { forgetRemoteReveals, sessionRemoteReveals, thread$ } from './thread'

// How the reader left each conversation, for the session: where it was
// scrolled and, in the traditional layout, which messages were open. Kept
// outside the conversation pane so leaving a thread — another tab, another
// thread in the list, the pane remounting between mail and a board — and
// coming back shows it as it was left.
//
// Everything here is plain data on purpose: a snapshot is what would carry a
// conversation's view to another window, whose webview shares none of this
// module's memory.

/** A scroll position as content rather than pixels (see ScrollAnchor), plus
 *  whether the reader had the view parked at the end of the thread, which
 *  decides whether returning restores the position or follows what arrived. */
export type SavedScrollAnchor = { messageId: string; offset: number; atBottom: boolean }

export interface ThreadView {
  /** Message ids the reader explicitly expanded or collapsed. Replaced, never
   *  mutated, so it can serve as an effect dependency. */
  overrides: Record<string, boolean>
  /** The reader's last expand-all / collapse-all in this thread. Messages
   *  without an override of their own follow it, ones loaded later included. */
  expandAll: { expanded: boolean } | null
  /** Messages that were unread when they first rendered. Scroll-driven read
   *  marking clears the live flag while the reader reads, so a default derived
   *  from it would collapse open messages out from under them. */
  unreadOnArrival: Set<string>
  /** Where the reader was, last time the view was measured. */
  scroll: SavedScrollAnchor | null
  /** Whether opening the thread should go back to `scroll`. Set when the
   *  reader leaves; consumed by the next positioning, or dropped by a direct
   *  jump to a message, which decides the position itself. */
  restoreScroll: boolean
}

export interface ThreadViewSnapshot {
  overrides: Record<string, boolean>
  expandAll: { expanded: boolean } | null
  unreadOnArrival: string[]
  scroll: SavedScrollAnchor | null
  /** Messages whose blocked remote content the reader revealed. */
  revealedRemote: string[]
}

// Current-navigation history is bounded; mounted views are never evicted, and
// tab-owned entries live until their tab closes rather than competing with it.
const MAX_THREADS = 200
const views = new Map<string, ThreadView>()
const owners = new Map<string, number>()
const tabViews = new Set<string>()
const discarded = new Set<string>()

function evictThreadViews() {
  let count = [...views.keys()].filter((key) => !tabViews.has(key)).length
  for (const key of views.keys()) {
    if (count <= MAX_THREADS) break
    if (owners.has(key) || tabViews.has(key)) continue
    views.delete(key)
    count--
  }
}

/** Protects a mounted view, including one hidden behind another tab. */
export function retainThreadView(key: string, tabOwned = false) {
  owners.set(key, (owners.get(key) ?? 0) + 1)
  if (tabOwned) tabViews.add(key)
  discarded.delete(key)
  threadView(key)
  return () => {
    // Child scroll cleanups still save their anchor during this unmount. Release
    // ownership after those run, so a closing tab cannot recreate its entry.
    queueMicrotask(() => {
      const count = owners.get(key)
      if (!count) return
      if (count > 1) owners.set(key, count - 1)
      else {
        owners.delete(key)
        if (discarded.delete(key)) {
          views.delete(key)
          tabViews.delete(key)
        }
      }
      evictThreadViews()
    })
  }
}

/** Drop a closed tab now, or after its mounted view has saved on teardown. */
export function forgetThreadView(key: string) {
  if (owners.has(key)) discarded.add(key)
  else {
    views.delete(key)
    tabViews.delete(key)
  }
}

function emptyView(): ThreadView {
  return { overrides: {}, expandAll: null, unreadOnArrival: new Set(), scroll: null, restoreScroll: false }
}

/** The thread's view state, created on first use. */
export function threadView(threadId: string): ThreadView {
  const view = views.get(threadId) ?? emptyView()
  views.delete(threadId)
  views.set(threadId, view)
  evictThreadViews()
  return view
}

/** Everything needed to show the thread as the reader left it elsewhere.
 *  `messageIds` are the thread's messages: remote reveals are kept by message,
 *  not by thread. */
export function snapshotThreadView(threadId: string, messageIds: readonly string[]): ThreadViewSnapshot {
  const view = views.get(threadId) ?? emptyView()
  const revealed = sessionRemoteReveals()
  return {
    overrides: { ...view.overrides },
    expandAll: view.expandAll ? { ...view.expandAll } : null,
    unreadOnArrival: [...view.unreadOnArrival],
    scroll: view.scroll ? { ...view.scroll } : null,
    revealedRemote: messageIds.filter((id) => revealed[id]),
  }
}

/** Takes a snapshot in, so the thread opens the way it was left. */
export function restoreThreadView(threadId: string, snapshot: ThreadViewSnapshot) {
  const view = threadView(threadId)
  view.overrides = { ...snapshot.overrides }
  view.expandAll = snapshot.expandAll ? { ...snapshot.expandAll } : null
  view.unreadOnArrival = new Set(snapshot.unreadOnArrival)
  view.scroll = snapshot.scroll ? { ...snapshot.scroll } : null
  view.restoreScroll = !!snapshot.scroll
  for (const id of snapshot.revealedRemote) thread$.revealedRemote[id].set(true)
}

/** Drops every thread's view state and remote reveals: after an account is
 *  removed, and in tests. */
export function forgetThreadViews() {
  views.clear()
  owners.clear()
  tabViews.clear()
  discarded.clear()
  forgetRemoteReveals()
}
