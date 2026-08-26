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
import {
  BASE_ROOT_FONT_SIZE,
  DEFAULT_FONT_SCALE,
  MAX_MESSAGE_FONT_SCALE,
  clampFontScale,
  clampMessageFontScale,
  fontStack,
  sanitizeFontChoice,
  sanitizeFontScale,
} from '../lib/fonts'
import {
  sanitizeShortcutOverrides,
  setShortcutOverrides,
  type Chord,
  type ShortcutId,
  type ShortcutOverrides,
} from '../lib/shortcuts'
import type { Account, ChatWallpaper } from '../types'
import { normalizeI18nLanguage, resolveI18nLanguageFromWebLocale, type SupportedI18nLanguage } from '../lib/i18n'

// Persisted user settings. This module maps 1:1 to the `settings` DB table: each
// field is one row, keyed by `DB_KEY`. Persistence is centralized here (a single
// root listener) so individual fields never wire up their own save logic.

/** How the quick reply composer sends: bare Enter, or Cmd/Ctrl+Enter. */
export type SendShortcut = 'enter' | 'mod_enter'

/**
 * How a thread's messages are laid out.
 * 'chat': left/right chat bubbles, every message expanded.
 * 'traditional': full-width stacked messages, collapsed to a one-line summary
 * except the newest and the unread ones (the classic mail-client reading view).
 */
export type ConversationLayout = 'chat' | 'traditional'
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

/** Proxy transport, or 'off' for direct connections. */
export type ProxyMode = 'off' | 'http' | 'socks5'

/**
 * Proxy endpoint, shared by the app-wide setting and the per-account override.
 * An empty `username` means the proxy needs no authentication.
 */
export type ProxySettings = {
  mode: ProxyMode
  host: string
  /** 0 while the field is empty; the core treats that as "no proxy". */
  port: number
  username: string
  password: string
}

export const EMPTY_PROXY: ProxySettings = {
  mode: 'off',
  host: '',
  port: 0,
  username: '',
  password: '',
}

export type Settings = {
  /** Active built-in or custom theme id. The theme's appearance controls light/dark mode. */
  themeId: string
  /** User-created themes (see lib/themes.ts). */
  customThemes: CustomTheme[]
  /** Interface font: '' for Inter, a lib/fonts option id, or a typed family name. */
  fontFamily: string
  /** Font for message bodies; '' follows the interface font. */
  messageFontFamily: string
  /** App-wide text size, in percent of the default (see lib/fonts). */
  fontScale: number
  /** Message body text size, in percent, applied on top of `fontScale`. */
  messageFontScale: number
  showRealAvatars: boolean
  /** Whether to overlay an inbox unread-count badge on side navigation account avatars. */
  showUnreadAccountBadge: boolean
  sendShortcut: SendShortcut
  /** Chat bubbles or the traditional stacked reading view (desktop only). */
  conversationLayout: ConversationLayout
  /** Whether native spell checking is requested in composer prose fields. */
  spellCheck: boolean
  /**
   * App-wide signature HTML, inserted into new messages and replies. Accounts
   * follow this unless they carry their own override (see Account.signature).
   * Empty means "no signature".
   */
  signature: string
  /**
   * Sender addresses (bare, lowercased) whose remote content always loads,
   * whatever an account's own "load remote images" toggle says. Grown from the
   * "Always allow from …" action on a message with blocked remote content.
   */
  remoteImageSenders: string[]
  /** Ordered user-created kanban boards. */
  kanbanBoards: KanbanBoard[]
  threadListWidth: number
  kanbanPaneWidth: number
  /** Pixel width used by every expanded kanban column. */
  kanbanColumnWidth: number
  /**
   * Whether the board keeps its horizontal scroll position while a card or
   * column is being dragged, instead of auto-scrolling toward the pointer.
   */
  kanbanLockScroll: boolean
  /** kanbanColumnKey -> whether the column is collapsed to a vertical bar. */
  kanbanMinimizedColumns: Record<string, boolean>
  /** Account ids hidden from the desktop side navigation. */
  hiddenSideNavAccounts: string[]
  /** Whether the synthetic unified inbox appears in the desktop side navigation. */
  showUnifiedInboxInSideNav: boolean
  /** Whether to poll for new releases in the background (see states/update.ts). */
  autoUpdateCheck: boolean
  /** Version whose update banner the user dismissed, so it doesn't nag. */
  dismissedUpdateVersion: string | null
  language: SupportedI18nLanguage | null
  /** Rebound keyboard shortcuts, keyed by shortcut id (see lib/shortcuts.ts). */
  shortcutOverrides: ShortcutOverrides
  /**
   * App-wide proxy for mail sockets, feed fetches and OAuth calls. Accounts
   * follow this unless they carry their own override (see AccountProxyCard).
   */
  proxy: ProxySettings
}

export const KANBAN_COLUMN_DEFAULT_WIDTH = 360
export const KANBAN_COLUMN_MIN_WIDTH = 240
export const KANBAN_COLUMN_MAX_WIDTH = 700

// The field <-> DB row mapping. Add a field here and it persists automatically;
// nothing else needs to change.
const DB_KEY = {
  themeId: 'theme_id',
  customThemes: 'custom_themes',
  fontFamily: 'font_family',
  messageFontFamily: 'message_font_family',
  fontScale: 'font_scale',
  messageFontScale: 'message_font_scale',
  showRealAvatars: 'show_real_avatars',
  showUnreadAccountBadge: 'show_unread_account_badge',
  sendShortcut: 'send_shortcut',
  conversationLayout: 'conversation_layout',
  spellCheck: 'spell_check',
  signature: 'signature',
  remoteImageSenders: 'remote_image_senders',
  kanbanBoards: 'kanban_boards',
  threadListWidth: 'thread_list_width',
  kanbanPaneWidth: 'kanban_pane_width',
  kanbanColumnWidth: 'kanban_column_width',
  kanbanLockScroll: 'kanban_lock_scroll',
  kanbanMinimizedColumns: 'kanban_minimized_columns',
  hiddenSideNavAccounts: 'hidden_sidenav_accounts',
  showUnifiedInboxInSideNav: 'show_unified_inbox_in_sidenav',
  autoUpdateCheck: 'auto_update_check',
  dismissedUpdateVersion: 'dismissed_update_version',
  language: 'language',
  shortcutOverrides: 'shortcut_overrides',
  proxy: 'proxy',
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

// Typography, like the theme, is painted before the DB rows arrive, so it keeps
// its own localStorage mirror to avoid a reflow from the default font/size to
// the chosen one. The DB rows stay authoritative.
const FONT_CACHE_KEY = 'meron-font-cache'

type FontSelection = Pick<Settings, 'fontFamily' | 'messageFontFamily' | 'fontScale' | 'messageFontScale'>

const DEFAULT_FONTS: FontSelection = {
  fontFamily: '',
  messageFontFamily: '',
  fontScale: DEFAULT_FONT_SCALE,
  messageFontScale: DEFAULT_FONT_SCALE,
}

// The chosen families feed the --font-sans / --font-message chains in index.css
// (which append the locale's CJK stack and the generic fallbacks), and the text
// size scales the root font size the rem-based text utilities are sized against.
function applyFontSelection(fonts: FontSelection) {
  const root = document.documentElement
  const ui = fontStack(fonts.fontFamily)
  const message = fontStack(fonts.messageFontFamily)
  // Clearing the var (rather than writing the default) lets the index.css
  // fallback paint, so devtools shows one source for the default typography.
  if (ui) root.style.setProperty('--me-font-ui', ui)
  else root.style.removeProperty('--me-font-ui')
  if (message) root.style.setProperty('--me-font-message', message)
  else root.style.removeProperty('--me-font-message')

  const scale = clampFontScale(fonts.fontScale)
  if (scale === DEFAULT_FONT_SCALE) root.style.removeProperty('font-size')
  else root.style.fontSize = `${(BASE_ROOT_FONT_SIZE * scale) / 100}px`

  // Message bodies multiply their own size on top (the app-wide size already
  // reaches them through the root font size). The frames can't see this var —
  // they get a pixel size baked into their stylesheet instead.
  const messageScale = clampMessageFontScale(fonts.messageFontScale)
  if (messageScale === DEFAULT_FONT_SCALE) root.style.removeProperty('--me-message-scale')
  else root.style.setProperty('--me-message-scale', String(messageScale / 100))
}

function bootstrapFontSelection(): FontSelection {
  try {
    const raw = localStorage.getItem(FONT_CACHE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Record<string, unknown>
      return {
        fontFamily: sanitizeFontChoice(parsed.fontFamily) ?? '',
        messageFontFamily: sanitizeFontChoice(parsed.messageFontFamily) ?? '',
        fontScale: sanitizeFontScale(parsed.fontScale) ?? DEFAULT_FONT_SCALE,
        messageFontScale: sanitizeFontScale(parsed.messageFontScale, MAX_MESSAGE_FONT_SCALE) ?? DEFAULT_FONT_SCALE,
      }
    }
  } catch {
    // Corrupt cache: fall through to defaults; the DB hydrate will repair it.
  }
  return DEFAULT_FONTS
}

const fontBootstrap = bootstrapFontSelection()
applyFontSelection(fontBootstrap)

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
  fontFamily: fontBootstrap.fontFamily,
  messageFontFamily: fontBootstrap.messageFontFamily,
  fontScale: fontBootstrap.fontScale,
  messageFontScale: fontBootstrap.messageFontScale,
  showRealAvatars: false,
  showUnreadAccountBadge: false,
  sendShortcut: 'mod_enter',
  conversationLayout: 'chat',
  spellCheck: true,
  signature: '',
  remoteImageSenders: [],
  kanbanBoards: [],
  threadListWidth: 350,
  kanbanPaneWidth: 33,
  kanbanColumnWidth: KANBAN_COLUMN_DEFAULT_WIDTH,
  kanbanLockScroll: false,
  kanbanMinimizedColumns: {},
  hiddenSideNavAccounts: [],
  showUnifiedInboxInSideNav: true,
  autoUpdateCheck: true,
  dismissedUpdateVersion: null,
  language: null,
  shortcutOverrides: {},
  proxy: EMPTY_PROXY,
})

// lib/shortcuts resolves chords from a local mirror, so keep it in step with the
// persisted overrides (hydration included — onChange fires for those too).
settings$.shortcutOverrides.onChange(({ value }) => setShortcutOverrides(value))

/** Repaint typography and refresh its paint-time cache. */
function syncFonts() {
  const fonts: FontSelection = {
    fontFamily: settings$.fontFamily.peek(),
    messageFontFamily: settings$.messageFontFamily.peek(),
    fontScale: settings$.fontScale.peek(),
    messageFontScale: settings$.messageFontScale.peek(),
  }
  applyFontSelection(fonts)
  try {
    localStorage.setItem(FONT_CACHE_KEY, JSON.stringify(fonts))
  } catch {
    // Storage unavailable: only the next first-paint hint is lost.
  }
}
settings$.fontFamily.onChange(syncFonts)
settings$.messageFontFamily.onChange(syncFonts)
settings$.fontScale.onChange(syncFonts)
settings$.messageFontScale.onChange(syncFonts)

// Suppress persistence while applying values loaded from the DB, so hydration
// doesn't immediately echo them back.
let hydrating = false

// The sequence stamp of the last write sent for each DB key. The sidecar runs
// each request on its own task, so two writes to one row can land out of order —
// a whole-value row (the remote-content allowlist, the board list) would then
// resurrect the value the user just replaced. Stamping instead of queueing keeps
// every write on the wire immediately, so nothing is left pending behind a slow
// call: the sidecar applies the newest stamp and drops the straggler.
const writeSeq = new Map<string, number>()

// These counters restart at 1 whenever this module is re-evaluated — a WebView
// reload, which the sidecar outlives — so each session labels its writes, and
// boot hands this label to the sidecar with its prefs read (`app.prefsGet`) to
// take over write ordering from the session before it. Without that, a reloaded
// window's edits would be dropped as stragglers, or a request left over from
// before the reload could overwrite them.
export const WRITE_SESSION = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`

function persistSetting(key: string, value: unknown) {
  const seq = (writeSeq.get(key) ?? 0) + 1
  writeSeq.set(key, seq)
  void invoke('app.prefsSet', { key, value, seq, session: WRITE_SESSION }).catch(() => {})
}

// The single persistence path: when a field changes, write just that field to
// its row. Replaces the old per-field onChange handlers scattered across states.
settings$.onChange(({ changes }) => {
  if (hydrating) return
  const seen = new Set<string>()
  for (const change of changes) {
    const field = change.path[0] as keyof Settings | undefined
    if (!field || seen.has(field) || !(field in DB_KEY)) continue
    seen.add(field)
    persistSetting(DB_KEY[field], settings$[field].get())
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

  // The webview doesn't paint the native title bar, so hand the appearance to
  // the backend as well and let it tint the window chrome to match.
  void invoke('window.setAppearance', { dark: def.appearance === 'dark' }).catch(() => {})
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

/**
 * Coerce a stored or user-entered proxy into the canonical shape. Returns null
 * for anything unrecognizable so hydration leaves the current value alone.
 */
export function sanitizeProxy(raw: unknown): ProxySettings | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null
  const v = raw as Record<string, unknown>
  const mode = v.mode
  if (mode !== 'off' && mode !== 'http' && mode !== 'socks5') return null
  const port = typeof v.port === 'number' && Number.isFinite(v.port) ? Math.trunc(v.port) : 0
  return {
    mode,
    host: typeof v.host === 'string' ? v.host.trim() : '',
    port: port > 0 && port <= 65535 ? port : 0,
    username: typeof v.username === 'string' ? v.username : '',
    password: typeof v.password === 'string' ? v.password : '',
  }
}

/** Whether a proxy is configured well enough to actually be used. */
export function isProxyUsable(proxy: ProxySettings): boolean {
  return proxy.mode !== 'off' && proxy.host.trim() !== '' && proxy.port > 0
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

// The form the allowlist stores and compares: bare address, lowercased. Mirrors
// `normalize_sender` in the sidecar so both ends agree on membership.
export function normalizeSenderAddr(addr: string): string {
  const trimmed = addr.trim()
  const open = trimmed.lastIndexOf('<')
  const close = trimmed.lastIndexOf('>')
  const bare = open >= 0 && close > open ? trimmed.slice(open + 1, close) : trimmed
  return bare.trim().toLowerCase()
}

// Allow (or stop allowing) remote content from one sender address. Unlike an
// account's "load remote images" toggle this is additive and app-wide: mail
// from `addr` loads its remote content in every account.
export function setRemoteImageSender(addr: string, allowed: boolean) {
  const address = normalizeSenderAddr(addr)
  if (!address) return
  const senders = settings$.remoteImageSenders.peek().filter((sender) => sender !== address)
  settings$.remoteImageSenders.set(allowed ? [...senders, address] : senders)
}

export function isAccountHiddenFromSideNav(accountId: string): boolean {
  return settings$.hiddenSideNavAccounts.peek().includes(accountId)
}

export function setAccountSideNavHidden(accountId: string, hidden: boolean) {
  const current = settings$.hiddenSideNavAccounts.peek()
  const has = current.includes(accountId)
  if (hidden === has) return
  settings$.hiddenSideNavAccounts.set(hidden ? [...current, accountId] : current.filter((id) => id !== accountId))
}

export function visibleSideNavAccounts(accounts: Account[]): Account[] {
  const hidden = new Set(settings$.hiddenSideNavAccounts.peek())
  return accounts.filter((account) => !hidden.has(account.id))
}

/** Rebind a shortcut. Binding it back to its default clears the override. */
export function setShortcutBinding(id: ShortcutId, chord: Chord) {
  const next = { ...settings$.shortcutOverrides.peek(), [id]: chord }
  settings$.shortcutOverrides.set(sanitizeShortcutOverrides(next) ?? {})
}

/** Restore one shortcut's default chord. */
export function resetShortcutBinding(id: ShortcutId) {
  const current = settings$.shortcutOverrides.peek()
  if (!current[id]) return
  const next = { ...current }
  delete next[id]
  settings$.shortcutOverrides.set(next)
}

/** Restore every shortcut's default chord. */
export function resetAllShortcutBindings() {
  if (Object.keys(settings$.shortcutOverrides.peek()).length === 0) return
  settings$.shortcutOverrides.set({})
}

export function setUnifiedInboxSideNavVisible(visible: boolean) {
  settings$.showUnifiedInboxInSideNav.set(visible)
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

    const fontFamily = sanitizeFontChoice(prefs[DB_KEY.fontFamily])
    if (fontFamily !== null) settings$.fontFamily.set(fontFamily)
    const messageFontFamily = sanitizeFontChoice(prefs[DB_KEY.messageFontFamily])
    if (messageFontFamily !== null) settings$.messageFontFamily.set(messageFontFamily)
    const fontScale = sanitizeFontScale(prefs[DB_KEY.fontScale])
    if (fontScale !== null) settings$.fontScale.set(fontScale)
    const messageFontScale = sanitizeFontScale(prefs[DB_KEY.messageFontScale], MAX_MESSAGE_FONT_SCALE)
    if (messageFontScale !== null) settings$.messageFontScale.set(messageFontScale)

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

    const conversationLayout = prefs[DB_KEY.conversationLayout]
    if (conversationLayout === 'chat' || conversationLayout === 'traditional') {
      settings$.conversationLayout.set(conversationLayout)
    }

    if (typeof prefs[DB_KEY.spellCheck] === 'boolean') {
      settings$.spellCheck.set(prefs[DB_KEY.spellCheck] as boolean)
    }

    if (typeof prefs[DB_KEY.signature] === 'string') {
      settings$.signature.set(prefs[DB_KEY.signature] as string)
    }

    // Normalize before deduping: "News <News@example.com>" and
    // "news@example.com" are one sender to the core, so they must be one row
    // here too — duplicates would also collide as React keys in the list.
    const storedSenders = prefs[DB_KEY.remoteImageSenders]
    const remoteImageSenders = sanitizeStringArray(
      Array.isArray(storedSenders)
        ? storedSenders.map((addr) => (typeof addr === 'string' ? normalizeSenderAddr(addr) : addr))
        : storedSenders,
    )
    if (remoteImageSenders) settings$.remoteImageSenders.set(remoteImageSenders)

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

    if (typeof prefs[DB_KEY.kanbanLockScroll] === 'boolean') {
      settings$.kanbanLockScroll.set(prefs[DB_KEY.kanbanLockScroll] as boolean)
    }

    const minimizedColumns = sanitizeBooleanMap(prefs[DB_KEY.kanbanMinimizedColumns])
    if (minimizedColumns) settings$.kanbanMinimizedColumns.set(minimizedColumns)

    const proxy = sanitizeProxy(prefs[DB_KEY.proxy])
    if (proxy) settings$.proxy.set(proxy)

    const hiddenSideNavAccounts = sanitizeStringArray(prefs[DB_KEY.hiddenSideNavAccounts])
    if (hiddenSideNavAccounts) settings$.hiddenSideNavAccounts.set(hiddenSideNavAccounts)

    if (typeof prefs[DB_KEY.showUnifiedInboxInSideNav] === 'boolean') {
      settings$.showUnifiedInboxInSideNav.set(prefs[DB_KEY.showUnifiedInboxInSideNav] as boolean)
    }

    if (typeof prefs[DB_KEY.autoUpdateCheck] === 'boolean') {
      settings$.autoUpdateCheck.set(prefs[DB_KEY.autoUpdateCheck] as boolean)
    }

    const dismissedUpdate = prefs[DB_KEY.dismissedUpdateVersion]
    if (typeof dismissedUpdate === 'string' || dismissedUpdate === null) {
      settings$.dismissedUpdateVersion.set(dismissedUpdate)
    }

    const language = normalizeI18nLanguage(prefs[DB_KEY.language] as string | null | undefined)
    settings$.language.set(language)

    const shortcutOverrides = sanitizeShortcutOverrides(prefs[DB_KEY.shortcutOverrides])
    if (shortcutOverrides) settings$.shortcutOverrides.set(shortcutOverrides)
  } finally {
    hydrating = false
  }
  // Re-apply once the stored theme is in: the module-load apply above ran
  // before the Wails bindings existed, so its window.setAppearance was lost,
  // and hydrating a theme identical to the current one fires no onChange.
  applyActiveTheme()
}
