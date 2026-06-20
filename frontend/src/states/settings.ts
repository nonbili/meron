import { observable } from '@legendapp/state'
import { invoke } from '../lib/bridge'
import {
  DEFAULT_LIGHT_ID,
  THEME_TOKEN_KEYS,
  TOKEN_CSS_VAR,
  builtinTheme,
  defaultThemeId,
  sanitizeCustomThemes,
  type CustomTheme,
  type ThemeDef,
} from '../lib/themes'
import { sanitizeChatWallpaper } from '../lib/wallpapers'
import type { Account, ChatWallpaper } from '../types'
import { normalizeI18nLanguage, resolveI18nLanguageFromWebLocale, type SupportedI18nLanguage } from '../lib/i18n'

// Persisted user settings. This module maps 1:1 to the `settings` DB table: each
// field is one row, keyed by `DB_KEY`. Persistence is centralized here (a single
// root listener) so individual fields never wire up their own save logic.

/** How the quick reply composer sends: bare Enter, or Cmd/Ctrl+Enter. */
export type SendShortcut = 'enter' | 'mod_enter'
export type KanbanBoardColumn = {
  accountId: string
  folderId: string
}
export type KanbanBoard = {
  id: string
  name: string
  columns: KanbanBoardColumn[]
  /** Custom rail/header image, e.g. "/media/avatars/kb-…/<uuid>.png". Unset = Columns3 tile. */
  avatarUrl?: string
  /** Background behind the board's columns. Unset = plain theme surface. */
  wallpaper?: ChatWallpaper | null
}

export type Settings = {
  /** Active built-in or custom theme id. The theme's appearance controls light/dark mode. */
  themeId: string
  /** User-created themes (see lib/themes.ts). */
  customThemes: CustomTheme[]
  showRealAvatars: boolean
  /** Whether to overlay an inbox unread-count badge on sidebar account avatars. */
  showUnreadAccountBadge: boolean
  sendShortcut: SendShortcut
  /** Ordered user-created kanban boards. */
  kanbanBoards: KanbanBoard[]
  threadListWidth: number
  kanbanPaneWidth: number
  /** Pixel width used by every expanded kanban column. */
  kanbanColumnWidth: number
  /** kanbanColumnKey -> whether the column is collapsed to a vertical bar. */
  kanbanMinimizedColumns: Record<string, boolean>
  /** Account ids hidden from the desktop left sidebar. */
  hiddenSidebarAccounts: string[]
  /** Whether the synthetic unified inbox appears in the desktop left sidebar. */
  showUnifiedInboxInSidebar: boolean
  /** Whether the Starred view button appears in the desktop left sidebar. */
  showStarredInSidebar: boolean
  language: SupportedI18nLanguage | null
}

export const KANBAN_COLUMN_DEFAULT_WIDTH = 360
export const KANBAN_COLUMN_MIN_WIDTH = 240
export const KANBAN_COLUMN_MAX_WIDTH = 700

// The field <-> DB row mapping. Add a field here and it persists automatically;
// nothing else needs to change.
const DB_KEY = {
  themeId: 'theme_id',
  customThemes: 'custom_themes',
  showRealAvatars: 'show_real_avatars',
  showUnreadAccountBadge: 'show_unread_account_badge',
  sendShortcut: 'send_shortcut',
  kanbanBoards: 'kanban_boards',
  threadListWidth: 'thread_list_width',
  kanbanPaneWidth: 'kanban_pane_width',
  kanbanColumnWidth: 'kanban_column_width',
  kanbanMinimizedColumns: 'kanban_minimized_columns',
  hiddenSidebarAccounts: 'hidden_sidebar_accounts',
  showUnifiedInboxInSidebar: 'show_unified_inbox_in_sidebar',
  showStarredInSidebar: 'show_starred_in_sidebar',
  language: 'language',
} satisfies Record<keyof Settings, string>

/** Keys to request from `app.prefsGet` on boot. */
export const SETTINGS_DB_KEYS = Object.values(DB_KEY)

const isMac = /mac|iphone|ipad|ipod/i.test(navigator.userAgent + ' ' + (navigator.platform ?? ''))

/** Human-readable key combo for a send shortcut, e.g. "Enter" or "⌘+Enter". */
export function sendShortcutLabel(shortcut: SendShortcut): string {
  if (shortcut === 'mod_enter') return isMac ? '⌘+Enter' : 'Ctrl+Enter'
  return 'Enter'
}

/**
 * Whether a keydown in the composer should send, given the active shortcut.
 * Enter mode: bare Enter (Shift+Enter inserts a newline).
 * Cmd/Ctrl+Enter mode: Enter inserts a newline, the modifier combo sends.
 */
export function isSendKey(
  e: { key: string; shiftKey: boolean; ctrlKey: boolean; metaKey: boolean },
  shortcut: SendShortcut,
): boolean {
  if (e.key !== 'Enter') return false
  if (shortcut === 'mod_enter') return (e.metaKey || e.ctrlKey) && !e.shiftKey
  return !e.shiftKey && !e.metaKey && !e.ctrlKey
}

// Theme is the only setting read before boot finishes (the sidecar load is
// async), so it keeps a synchronous localStorage bootstrap to avoid a
// light-on-dark first-paint flash. The DB rows stay authoritative.
const THEME_CACHE_KEY = 'meron-theme-cache'

function bootstrapThemeSelection(): Pick<Settings, 'themeId' | 'customThemes'> {
  try {
    const raw = localStorage.getItem(THEME_CACHE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Record<string, unknown>
      return {
        themeId: typeof parsed.themeId === 'string' && parsed.themeId ? parsed.themeId : DEFAULT_LIGHT_ID,
        customThemes: sanitizeCustomThemes(parsed.customThemes) ?? [],
      }
    }
  } catch {
    // Corrupt cache: fall through to defaults; the DB hydrate will repair it.
  }
  return { themeId: DEFAULT_LIGHT_ID, customThemes: [] }
}

const themeBootstrap = bootstrapThemeSelection()

// Like the theme, the active locale is reflected to <html lang>/dir so CJK glyph
// selection (:lang() in index.css) and RTL paint correctly. The DB row loads
// async, so a localStorage mirror lets us set it synchronously at first paint;
// it falls back to the OS locale until the DB hydrates. DB rows stay
// authoritative — this cache is only a paint-time hint.
const LANG_CACHE_KEY = 'meron-language-cache'

/** Reflect a locale to the document and refresh the paint-time cache. */
export function applyDocumentLanguage(lang: SupportedI18nLanguage) {
  const root = document.documentElement
  // Our codes use "_" (zh_Hant, pt_BR); the lang attribute wants BCP-47 "-".
  root.lang = lang.replace('_', '-')
  root.dir = lang === 'ar' ? 'rtl' : 'ltr'
  try {
    localStorage.setItem(LANG_CACHE_KEY, lang)
  } catch {
    // Storage unavailable: the attribute is still set, only the cache is skipped.
  }
}

;(function bootstrapDocumentLanguage() {
  try {
    const cached = normalizeI18nLanguage(localStorage.getItem(LANG_CACHE_KEY))
    applyDocumentLanguage(cached ?? resolveI18nLanguageFromWebLocale(navigator.language) ?? 'en')
  } catch {
    // Best effort; useAppEffects re-applies once the DB-backed language resolves.
  }
})()

export const settings$ = observable<Settings>({
  themeId: themeBootstrap.themeId,
  customThemes: themeBootstrap.customThemes,
  showRealAvatars: false,
  showUnreadAccountBadge: false,
  sendShortcut: 'mod_enter',
  kanbanBoards: [],
  threadListWidth: 350,
  kanbanPaneWidth: 33,
  kanbanColumnWidth: KANBAN_COLUMN_DEFAULT_WIDTH,
  kanbanMinimizedColumns: {},
  hiddenSidebarAccounts: [],
  showUnifiedInboxInSidebar: true,
  showStarredInSidebar: false,
  language: null,
})

// Suppress persistence while applying values loaded from the DB, so hydration
// doesn't immediately echo them back.
let hydrating = false

// The single persistence path: when a field changes, write just that field to
// its row. Replaces the old per-field onChange handlers scattered across states.
settings$.onChange(({ changes }) => {
  if (hydrating) return
  const seen = new Set<string>()
  for (const change of changes) {
    const field = change.path[0] as keyof Settings | undefined
    if (!field || seen.has(field) || !(field in DB_KEY)) continue
    seen.add(field)
    void invoke('app.prefsSet', { key: DB_KEY[field], value: settings$[field].get() }).catch(() => {})
  }
})

/** The theme that should currently be painted, after fallbacks. */
export function resolveThemeDef(): ThemeDef {
  const id = settings$.themeId.peek()
  const custom = settings$.customThemes.peek().find((theme) => theme.id === id)
  if (custom) return custom
  const builtin = builtinTheme(id)
  // A stale id (deleted custom theme, renamed builtin) falls back to Meron Light.
  if (builtin) return builtin
  return builtinTheme(DEFAULT_LIGHT_ID)!
}

// The active theme is reflected to the DOM (the `.dark` class drives Tailwind
// `dark:` variants; inline vars on <html> override the :root/.dark defaults
// from index.css) and to the localStorage bootstrap cache.
function applyActiveTheme() {
  const def = resolveThemeDef()
  const root = document.documentElement
  root.classList.toggle('dark', def.appearance === 'dark')
  // The two index.css defaults paint via the cascade; clearing the inline vars
  // (instead of re-setting them) keeps devtools and :root overrides sane.
  const isDefault = def.id === defaultThemeId(def.appearance)
  for (const key of THEME_TOKEN_KEYS) {
    if (isDefault) root.style.removeProperty(TOKEN_CSS_VAR[key])
    else root.style.setProperty(TOKEN_CSS_VAR[key], def.tokens[key])
  }

  localStorage.setItem(
    THEME_CACHE_KEY,
    JSON.stringify({
      themeId: settings$.themeId.peek(),
      customThemes: settings$.customThemes.peek(),
    }),
  )
}
applyActiveTheme()
settings$.themeId.onChange(applyActiveTheme)
// Editing the active custom theme must repaint live.
settings$.customThemes.onChange(applyActiveTheme)

/** Pick the active theme. Its appearance controls light/dark mode. */
export function selectTheme(def: ThemeDef) {
  settings$.themeId.set(def.id)
}

/** Add or replace a custom theme and make it the active choice for its appearance. */
export function upsertCustomTheme(theme: CustomTheme) {
  const current = settings$.customThemes.peek()
  const exists = current.some((item) => item.id === theme.id)
  settings$.customThemes.set(
    exists ? current.map((item) => (item.id === theme.id ? theme : item)) : [...current, theme],
  )
  selectTheme(theme)
}

/** Delete a custom theme; if it was selected, its appearance falls back to the default. */
export function deleteCustomTheme(id: string) {
  const current = settings$.customThemes.peek()
  const theme = current.find((item) => item.id === id)
  if (!theme) return
  settings$.customThemes.set(current.filter((item) => item.id !== id))
  if (settings$.themeId.peek() === id) settings$.themeId.set(DEFAULT_LIGHT_ID)
}

export function sanitizeKanbanBoards(raw: unknown): KanbanBoard[] | null {
  if (!Array.isArray(raw)) return null
  const out: KanbanBoard[] = []
  const seen = new Set<string>()
  for (const item of raw) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) continue
    const obj = item as Record<string, unknown>
    if (typeof obj.id !== 'string' || !obj.id || seen.has(obj.id)) continue
    const name = typeof obj.name === 'string' && obj.name.trim() ? obj.name.trim() : 'Kanban board'
    const columns = Array.isArray(obj.columns)
      ? obj.columns.flatMap((column): KanbanBoardColumn[] => {
          if (!column || typeof column !== 'object' || Array.isArray(column)) return []
          const col = column as Record<string, unknown>
          if (typeof col.accountId !== 'string' || typeof col.folderId !== 'string') return []
          if (!col.accountId || !col.folderId) return []
          return [{ accountId: col.accountId, folderId: col.folderId }]
        })
      : []
    seen.add(obj.id)
    const board: KanbanBoard = { id: obj.id, name, columns }
    // Board images are app-managed uploads; only paths under /media/avatars/
    // (written by account.writeAvatarFile) are accepted.
    if (typeof obj.avatarUrl === 'string' && obj.avatarUrl.startsWith('/media/avatars/')) {
      board.avatarUrl = obj.avatarUrl
    }
    const wallpaper = sanitizeChatWallpaper(obj.wallpaper)
    if (wallpaper) board.wallpaper = wallpaper
    out.push(board)
  }
  return out
}

export function clampKanbanColumnWidth(width: number): number {
  return Math.round(Math.min(KANBAN_COLUMN_MAX_WIDTH, Math.max(KANBAN_COLUMN_MIN_WIDTH, width)))
}

function sanitizeBooleanMap(raw: unknown): Record<string, boolean> | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null
  const out: Record<string, boolean> = {}
  for (const [key, value] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof value === 'boolean') out[key] = value
  }
  return out
}

function sanitizeStringArray(raw: unknown): string[] | null {
  if (!Array.isArray(raw)) return null
  const out: string[] = []
  const seen = new Set<string>()
  for (const value of raw) {
    if (typeof value !== 'string' || !value || seen.has(value)) continue
    seen.add(value)
    out.push(value)
  }
  return out
}

export function isAccountHiddenFromSidebar(accountId: string): boolean {
  return settings$.hiddenSidebarAccounts.peek().includes(accountId)
}

export function setAccountSidebarHidden(accountId: string, hidden: boolean) {
  const current = settings$.hiddenSidebarAccounts.peek()
  const has = current.includes(accountId)
  if (hidden === has) return
  settings$.hiddenSidebarAccounts.set(hidden ? [...current, accountId] : current.filter((id) => id !== accountId))
}

export function visibleSidebarAccounts(accounts: Account[]): Account[] {
  const hidden = new Set(settings$.hiddenSidebarAccounts.peek())
  return accounts.filter((account) => !hidden.has(account.id))
}

export function setUnifiedInboxSidebarVisible(visible: boolean) {
  settings$.showUnifiedInboxInSidebar.set(visible)
}

export function setStarredSidebarVisible(visible: boolean) {
  settings$.showStarredInSidebar.set(visible)
}

/** Apply persisted settings loaded from the DB (via `app.prefsGet`). */
export function hydrateSettings(prefs: Record<string, unknown>) {
  hydrating = true
  try {
    // Theme ids are validated for existence at resolve time (resolveThemeDef
    // falls back to the default), not here, so an id can survive its custom
    // theme arriving in a later hydrate.
    const themeId = prefs[DB_KEY.themeId]
    if (typeof themeId === 'string' && themeId) settings$.themeId.set(themeId)
    const customThemes = sanitizeCustomThemes(prefs[DB_KEY.customThemes])
    if (customThemes) settings$.customThemes.set(customThemes)

    if (typeof prefs[DB_KEY.showRealAvatars] === 'boolean') {
      settings$.showRealAvatars.set(prefs[DB_KEY.showRealAvatars] as boolean)
    }

    if (typeof prefs[DB_KEY.showUnreadAccountBadge] === 'boolean') {
      settings$.showUnreadAccountBadge.set(prefs[DB_KEY.showUnreadAccountBadge] as boolean)
    }

    const sendShortcut = prefs[DB_KEY.sendShortcut]
    if (sendShortcut === 'enter' || sendShortcut === 'mod_enter') {
      settings$.sendShortcut.set(sendShortcut)
    }

    const boards = sanitizeKanbanBoards(prefs[DB_KEY.kanbanBoards])
    if (boards) settings$.kanbanBoards.set(boards)

    const threadListWidth = prefs[DB_KEY.threadListWidth]
    if (typeof threadListWidth === 'number' && Number.isFinite(threadListWidth)) {
      settings$.threadListWidth.set(Math.min(560, Math.max(280, threadListWidth)))
    }

    const paneWidth = prefs[DB_KEY.kanbanPaneWidth]
    if (typeof paneWidth === 'number' && Number.isFinite(paneWidth)) {
      settings$.kanbanPaneWidth.set(paneWidth)
    }

    const columnWidth = prefs[DB_KEY.kanbanColumnWidth]
    if (typeof columnWidth === 'number' && Number.isFinite(columnWidth)) {
      settings$.kanbanColumnWidth.set(clampKanbanColumnWidth(columnWidth))
    }

    const minimizedColumns = sanitizeBooleanMap(prefs[DB_KEY.kanbanMinimizedColumns])
    if (minimizedColumns) settings$.kanbanMinimizedColumns.set(minimizedColumns)

    const hiddenSidebarAccounts = sanitizeStringArray(prefs[DB_KEY.hiddenSidebarAccounts])
    if (hiddenSidebarAccounts) settings$.hiddenSidebarAccounts.set(hiddenSidebarAccounts)

    if (typeof prefs[DB_KEY.showUnifiedInboxInSidebar] === 'boolean') {
      settings$.showUnifiedInboxInSidebar.set(prefs[DB_KEY.showUnifiedInboxInSidebar] as boolean)
    }

    if (typeof prefs[DB_KEY.showStarredInSidebar] === 'boolean') {
      settings$.showStarredInSidebar.set(prefs[DB_KEY.showStarredInSidebar] as boolean)
    }

    const language = normalizeI18nLanguage(prefs[DB_KEY.language] as string | null | undefined)
    settings$.language.set(language)
  } finally {
    hydrating = false
  }
}
