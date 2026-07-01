import { observable } from '@legendapp/state'
import type { Account, SystemCheck } from '../types'
import { invoke } from '../lib/bridge'
import { persistedField } from '../lib/sessionPref'

// Mostly ephemeral UI state: selections, view chrome, transient flags, and the
// one-time system check, reset every launch. A small subset — the last
// navigation (account / folder / filter) — is session state that's persisted
// here and restored at boot (see the bottom of this file). Server data lives in
// the domain state modules; user preferences live in states/settings.

export type FilterMode = 'all' | 'unread' | 'starred'

export const isFilterMode = (value: unknown): value is FilterMode =>
  value === 'all' || value === 'unread' || value === 'starred'
export type SetupMode = 'gmail' | 'outlook' | 'custom' | 'rss'
export type MobilePane = 'threads' | 'conversation'
export type ToastTone = 'success' | 'error'
export type EditFeed = { threadId: string; name: string; url?: string }
export type ConfirmTone = 'default' | 'danger'
export type ConfirmState = {
  id: string
  title: string
  message: string
  confirmLabel: string
  cancelLabel: string
  tone: ConfirmTone
}

export type BulkSelectionSurface = 'thread-list' | 'starred' | 'kanban'
export type BulkSelectionKind = 'mail' | 'feed'
export type BulkSelectionItem = {
  key: string
  groupKey: string
  threadId: string
  messageId?: string
  accountId: string
  folderId: string
  surface: BulkSelectionSurface
  kind: BulkSelectionKind
  unread: boolean
  starred: boolean
  draft: boolean
  trash: boolean
}

export const ui$ = observable({
  // Backend/system check, loaded once at boot.
  system: null as SystemCheck | null,
  // Current view selection.
  selectedAccount: '',
  selectedFolder: 'inbox',
  selectedThread: '',
  // Starred is a flat item list, so multiple rows can share selectedThread.
  selectedStarredItem: '',
  bulkSelection: {} as Record<string, BulkSelectionItem>,
  bulkAnchorKey: '',
  query: '',
  filterMode: 'all' as FilterMode,
  // Modals / panels.
  setupOpen: false,
  setupMode: 'gmail' as SetupMode,
  reconnectAccountId: '',
  settingsOpen: false,
  aboutOpen: false,
  changelogOpen: false,
  // Account id whose per-account settings panel is open ("" = closed).
  accountSettingsId: '',
  addFeedAccount: '',
  editFeed: null as EditFeed | null,
  // Command palette (⌘/Ctrl+K). Ephemeral: open flag, search query, and the
  // highlighted row index.
  paletteOpen: false,
  paletteQuery: '',
  paletteIndex: 0,
  // Bumped to ask the ThreadList to focus its "Search messages" box
  // (⌘/Ctrl+Shift+F). A nonce, not a boolean, so repeated presses re-focus even
  // when the value would otherwise be unchanged.
  globalSearchFocus: 0,
  // Keyboard-shortcuts cheat sheet (⌘/Ctrl+?).
  shortcutsOpen: false,
  // Bumped to ask the open conversation's quick-reply box to focus (the "r"
  // shortcut). Nonce, like globalSearchFocus.
  replyFocus: 0,
  // Responsive layout (narrow window) + transient flags.
  mobilePane: 'threads' as MobilePane,
  busy: false,
  toast: '',
  // Visual tone of the current toast: "success" shows a green check, "error" a
  // red alert. Defaults to success for the common confirmation case.
  toastTone: 'success' as ToastTone,
  // Optional one-shot undo paired with the current toast. Set by reversible
  // actions (archive/star/unread) so an accidental keystroke is recoverable.
  toastUndo: null as null | (() => void),
  confirm: null as ConfirmState | null,
})

let pendingConfirmResolve: ((confirmed: boolean) => void) | null = null

// Open the command palette, resetting the query and selection so it always
// starts fresh at the top.
export function openCommandPalette() {
  ui$.paletteQuery.set('')
  ui$.paletteIndex.set(0)
  ui$.paletteOpen.set(true)
}

export function closeCommandPalette() {
  ui$.paletteOpen.set(false)
}

// Ask the ThreadList to focus (and select) its search box.
export function focusGlobalSearch() {
  ui$.globalSearchFocus.set(ui$.globalSearchFocus.peek() + 1)
}

// Show a transient toast for ~2.2s. Clears only if it's still showing this
// message, so a newer toast isn't cut short. Pass tone "error" for failures.
export function showToast(msg: string, tone: ToastTone = 'success') {
  ui$.toastUndo.set(null)
  ui$.toastTone.set(tone)
  ui$.toast.set(msg)
  setTimeout(() => {
    if (ui$.toast.get() === msg) {
      ui$.toast.set('')
      ui$.toastUndo.set(null)
    }
  }, 2200)
}

// Like showToast but pairs the message with an Undo affordance. Given a longer
// (5s) window since the whole point is to catch an accidental action.
export function showUndoToast(msg: string, undo: () => void) {
  ui$.toastTone.set('success')
  ui$.toast.set(msg)
  ui$.toastUndo.set(() => undo)
  setTimeout(() => {
    if (ui$.toast.get() === msg) {
      ui$.toast.set('')
      ui$.toastUndo.set(null)
    }
  }, 5000)
}

// Run the pending undo (if any) and dismiss the toast.
export function runToastUndo() {
  const undo = ui$.toastUndo.peek()
  ui$.toast.set('')
  ui$.toastUndo.set(null)
  undo?.()
}

export function confirmAction(
  options:
    | string
    | {
        title?: string
        message: string
        confirmLabel?: string
        cancelLabel?: string
        tone?: ConfirmTone
      },
): Promise<boolean> {
  const next = typeof options === 'string' ? { message: options } : options

  pendingConfirmResolve?.(false)

  ui$.confirm.set({
    id: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
    title: next.title ?? 'Confirm action',
    message: next.message,
    confirmLabel: next.confirmLabel ?? 'Confirm',
    cancelLabel: next.cancelLabel ?? 'Cancel',
    tone: next.tone ?? 'default',
  })

  return new Promise((resolve) => {
    pendingConfirmResolve = resolve
  })
}

export function settleConfirm(confirmed: boolean) {
  const resolve = pendingConfirmResolve
  pendingConfirmResolve = null
  ui$.confirm.set(null)
  resolve?.(confirmed)
}

export function isWailsDesktopRuntime() {
  return Boolean((window as any).go?.main?.App?.Invoke)
}

export function selectedBulkItems(): BulkSelectionItem[] {
  return Object.values(ui$.bulkSelection.peek())
}

export function clearBulkSelection() {
  ui$.bulkSelection.set({})
  ui$.bulkAnchorKey.set('')
}

export function setBulkSelection(items: BulkSelectionItem[], anchorKey = items.at(-1)?.key ?? '') {
  const groupKey = items.find((item) => item.key === anchorKey)?.groupKey ?? items.at(-1)?.groupKey ?? ''
  const scopedItems = groupKey ? items.filter((item) => item.groupKey === groupKey) : items
  ui$.bulkSelection.set(Object.fromEntries(scopedItems.map((item) => [item.key, item])))
  ui$.bulkAnchorKey.set(anchorKey)
}

export function toggleBulkSelection(item: BulkSelectionItem) {
  const existing = selectedBulkItems()
  const sameGroup = existing.length === 0 || existing.every((selected) => selected.groupKey === item.groupKey)
  const current = sameGroup ? { ...ui$.bulkSelection.peek() } : {}
  if (current[item.key]) {
    delete current[item.key]
  } else {
    current[item.key] = item
  }
  ui$.bulkSelection.set(current)
  ui$.bulkAnchorKey.set(item.key)
}

// Focus the open conversation's quick-reply box (the "r" shortcut).
export function focusQuickReply() {
  ui$.replyFocus.set(ui$.replyFocus.peek() + 1)
}

// --- Session persistence: last navigation, restored on the next launch. ---

// The filter is an independent value, so it uses the generic persisted-field
// helper. Account + folder are coupled (a folder only restores under a valid
// account) and depend on the loaded accounts list, so they share one bespoke
// restore below rather than per-field seeding.
const filterSession = persistedField(ui$.filterMode, 'session_filter_mode', (raw) =>
  isFilterMode(raw) ? raw : undefined,
)

let restoringNav = false
function persistNav(key: string, value: string) {
  if (restoringNav) return
  void invoke('app.prefsSet', { key, value }).catch(() => {})
}
ui$.selectedAccount.onChange(({ value }) => persistNav('session_account', value))
ui$.selectedFolder.onChange(({ value }) => persistNav('session_folder', value))
ui$.selectedAccount.onChange(() => clearBulkSelection())
ui$.selectedFolder.onChange(() => clearBulkSelection())
ui$.query.onChange(() => clearBulkSelection())
ui$.filterMode.onChange(() => clearBulkSelection())

/** Prefs keys this module owns; boot requests them in its single prefsGet. */
export const UI_SESSION_KEYS = ['session_account', 'session_folder', filterSession.key]

/**
 * Seed ui$ navigation from the persisted last session. Call once at boot after
 * accounts are loaded. A saved account that no longer exists falls back to the
 * unified inbox; with no accounts at all, selection is cleared.
 */
export function restoreUiSession(prefs: Record<string, unknown>, accounts: Account[]) {
  restoringNav = true
  try {
    if (accounts.length === 0) {
      ui$.selectedAccount.set('')
    } else {
      const saved = prefs['session_account']
      const valid =
        typeof saved === 'string' &&
        (saved === 'unified' || saved === 'starred' || accounts.some((a) => a.id === saved))
      ui$.selectedAccount.set(valid ? (saved as string) : 'unified')
      const folder = prefs['session_folder']
      if (typeof folder === 'string' && folder) ui$.selectedFolder.set(folder)
    }
  } finally {
    restoringNav = false
  }
  filterSession.restore(prefs)
}
