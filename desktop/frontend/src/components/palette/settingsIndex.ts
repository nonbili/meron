import { settings$, appIconSupported$ } from '../../states/settings'
import { update$ } from '../../states/update'
import { windowChrome$ } from '../../lib/windowChrome'
import { isMac } from '../../lib/shortcuts'
import { offersMonochromeTrayIcon } from '../../lib/monochromeTrayIcon'
import { isWindowsPlatform } from '../../lib/platform'
import { isLocalAccountId } from '../../states/accounts'
import type { Account } from '../../types'

/** A Settings dialog page that isn't an account or a board. */
export type SettingsPage = 'general' | 'messages' | 'composer' | 'rules' | 'mcp' | 'encryption' | 'calendars'

export type SettingEntry = {
  page: SettingsPage
  /**
   * The i18n key of the row's (or group's) title. The dialog finds the row to
   * reveal by this title, so it must be the key the row renders.
   */
  titleKey: string
  /** Extra English words matched by the palette's search but not shown. */
  keywords?: string
  /** For rows the dialog shows only under some condition. */
  available?: () => boolean
}

const calendarOn = () => settings$.calendarEnabled.peek()

// The settings the command palette can jump to. Order is the order they appear
// in the dialog, so equally good matches list the way the pages read.
export const SETTINGS_INDEX: SettingEntry[] = [
  // General
  { page: 'general', titleKey: 'common.theme', keywords: 'appearance dark light color colour mode' },
  { page: 'general', titleKey: 'settings.language.label', keywords: 'locale translation' },
  { page: 'general', titleKey: 'settings.appearance.showSenderImages', keywords: 'avatars pictures photos' },
  { page: 'general', titleKey: 'settings.appearance.showThreadAvatars', keywords: 'gravatar compact density' },
  {
    page: 'general',
    titleKey: 'settings.appearance.appIconCustom',
    keywords: 'dock icon color colour',
    available: () => appIconSupported$.peek(),
  },
  {
    page: 'general',
    titleKey: 'settings.appearance.monochromeTrayIcon',
    keywords: 'tray menu bar icon',
    available: () => offersMonochromeTrayIcon(navigator.userAgent, navigator.platform ?? ''),
  },
  {
    page: 'general',
    titleKey: 'settings.appearance.showTaskbarUnreadBadge',
    keywords: 'taskbar unread badge count overlay icon',
    available: () => isWindowsPlatform(navigator.userAgent, navigator.platform ?? ''),
  },
  { page: 'general', titleKey: 'settings.appearance.uiFont', keywords: 'typography typeface font family' },
  { page: 'general', titleKey: 'settings.appearance.textSize', keywords: 'font size scale zoom' },
  { page: 'general', titleKey: 'settings.sideNav.showUnifiedInbox', keywords: 'sidebar rail all accounts' },
  {
    page: 'general',
    titleKey: 'settings.sideNav.showCalendar',
    keywords: 'sidebar rail',
    available: calendarOn,
  },
  { page: 'general', titleKey: 'settings.appearance.showUnreadAccountBadge', keywords: 'sidebar count' },
  { page: 'general', titleKey: 'settings.kanban.columnWidth', keywords: 'board' },
  { page: 'general', titleKey: 'settings.kanban.lockScroll', keywords: 'board' },
  { page: 'general', titleKey: 'settings.sections.network', keywords: 'proxy socks http' },
  { page: 'general', titleKey: 'shortcuts.title', keywords: 'keys bindings customize' },
  { page: 'general', titleKey: 'settings.window.closeToTray', keywords: 'quit minimize', available: () => !isMac },
  {
    page: 'general',
    titleKey: 'settings.window.integratedTitlebar',
    keywords: 'window decorations frame',
    available: () => windowChrome$.supported.peek(),
  },
  {
    page: 'general',
    titleKey: 'settings.updates.autoCheck',
    keywords: 'upgrade version',
    available: () => update$.status.peek().supported,
  },
  { page: 'general', titleKey: 'settings.backup.fileTitle', keywords: 'export import restore' },
  { page: 'general', titleKey: 'settings.storage.usageTitle', keywords: 'disk space cache database' },
  { page: 'general', titleKey: 'settings.storage.clearTitle', keywords: 'cache disk space' },
  { page: 'general', titleKey: 'settings.sections.logs', keywords: 'sync diagnostic debug' },

  // Messages
  { page: 'messages', titleKey: 'settings.appearance.conversationLayout', keywords: 'chat traditional bubbles' },
  {
    page: 'messages',
    titleKey: 'settings.appearance.chatFullMessages',
    keywords: 'expand',
    available: () => settings$.conversationLayout.peek() === 'chat',
  },
  { page: 'messages', titleKey: 'settings.appearance.darkMessageBodies', keywords: 'dark mode email html' },
  { page: 'messages', titleKey: 'settings.appearance.autoFitMessages', keywords: 'shrink wide zoom' },
  {
    page: 'messages',
    titleKey: 'settings.appearance.conversationFooter',
    keywords: 'quick reply box buttons toolbar action bar',
  },
  { page: 'messages', titleKey: 'settings.appearance.messageFont', keywords: 'typography typeface font family' },
  { page: 'messages', titleKey: 'settings.appearance.messageTextSize', keywords: 'font size scale zoom' },
  { page: 'messages', titleKey: 'tags.title', keywords: 'labels' },
  { page: 'messages', titleKey: 'settings.privacy.remoteSenders', keywords: 'remote images privacy tracking' },
  { page: 'messages', titleKey: 'settings.privacy.detectCodes', keywords: 'otp one-time verification 2fa' },

  // Composer
  { page: 'composer', titleKey: 'settings.composer.spellCheck', keywords: 'spelling' },
  { page: 'composer', titleKey: 'settings.composer.quoteReplies', keywords: 'quoted' },
  { page: 'composer', titleKey: 'settings.composer.sendMessageWith', keywords: 'enter shortcut send' },
  { page: 'composer', titleKey: 'settings.sections.signature', keywords: 'footer' },

  // Pages of their own
  { page: 'rules', titleKey: 'rules.title', keywords: 'filters automation' },
  { page: 'mcp', titleKey: 'mcp.title', keywords: 'ai assistant server' },
  { page: 'encryption', titleKey: 'crypto.peers.nav', keywords: 'pgp openpgp s/mime smime keys' },

  // Calendars & tasks
  { page: 'calendars', titleKey: 'settings.calendar.enable', keywords: 'events' },
  { page: 'calendars', titleKey: 'settings.tasks.enable', keywords: 'todo' },
  {
    page: 'calendars',
    titleKey: 'settings.calendar.reminderLead',
    keywords: 'alert notification event',
    available: calendarOn,
  },
  {
    page: 'calendars',
    titleKey: 'settings.calendar.allDayAlert',
    keywords: 'reminder notification event',
    available: calendarOn,
  },
  {
    page: 'calendars',
    titleKey: 'settings.calendars.accounts',
    keywords: 'caldav google tasks sync nextcloud fastmail icloud',
    available: () => settings$.calendarEnabled.peek() || settings$.tasksEnabled.peek(),
  },
]

/** Which accounts a per-account setting applies to, as the settings page splits them. */
export type AccountKind = 'mail' | 'rss' | 'local'

export function accountKind(account: Account): AccountKind {
  if (account.provider === 'rss' || account.auth_type === 'rss') return 'rss'
  return isLocalAccountId(account.id) ? 'local' : 'mail'
}

export type AccountSettingEntry = {
  titleKey: string
  keywords?: string
  kinds: AccountKind[]
  /** For rows that depend on more than the account's kind. */
  available?: (account: Account) => boolean
}

const ALL_KINDS: AccountKind[] = ['mail', 'rss', 'local']

// Settings each account has on its own page, in page order. Listed once per
// account they apply to.
export const ACCOUNT_SETTINGS_INDEX: AccountSettingEntry[] = [
  {
    titleKey: 'settings.account.serverAccount',
    keywords: 'imap smtp host port server',
    kinds: ['mail'],
    available: (account) => account.auth_type === 'password' && !account.needs_reconnect,
  },
  { titleKey: 'settings.account.displayName', keywords: 'rename name', kinds: ALL_KINDS },
  { titleKey: 'settings.account.senderName', keywords: 'from name', kinds: ['mail'] },
  { titleKey: 'settings.account.color', keywords: 'colour bar unified label', kinds: ALL_KINDS },
  { titleKey: 'settings.account.chatBackground', keywords: 'wallpaper', kinds: ALL_KINDS },
  { titleKey: 'settings.account.showInUnifiedInbox', kinds: ['mail', 'rss'] },
  { titleKey: 'settings.account.showInSideNav', keywords: 'sidebar rail hide', kinds: ALL_KINDS },
  { titleKey: 'settings.account.muteNotifications', keywords: 'silence', kinds: ['mail', 'rss'] },
  { titleKey: 'settings.account.pauseAccount', keywords: 'sync stop', kinds: ['mail', 'rss'] },
  { titleKey: 'settings.account.syncInterval', keywords: 'refresh poll', kinds: ['rss'] },
  { titleKey: 'settings.account.folderNotifications', keywords: 'alerts', kinds: ['mail'] },
  { titleKey: 'settings.account.loadRemoteImages', keywords: 'privacy tracking', kinds: ALL_KINDS },
  { titleKey: 'settings.account.conversationView', keywords: 'html plain text', kinds: ALL_KINDS },
  { titleKey: 'settings.account.saveSentCopies', keywords: 'sent folder', kinds: ['mail'] },
  { titleKey: 'crypto.account.title', keywords: 'pgp openpgp keys', kinds: ['mail'] },
  { titleKey: 'settings.sections.network', keywords: 'proxy socks http', kinds: ['mail'] },
  { titleKey: 'settings.account.aliases', keywords: 'identities from addresses', kinds: ['mail'] },
  { titleKey: 'settings.sections.signature', keywords: 'footer', kinds: ['mail'] },
  { titleKey: 'settings.feeds.opmlFile', keywords: 'import export subscriptions', kinds: ['rss'] },
  { titleKey: 'settings.sections.data', keywords: 'storage backup export restore', kinds: ['local'] },
]

// Settings each Kanban board has on its own page.
export const BOARD_SETTINGS_INDEX: { titleKey: string; keywords?: string }[] = [
  { titleKey: 'kanban.board.name', keywords: 'rename' },
  { titleKey: 'kanban.board.background', keywords: 'wallpaper' },
  { titleKey: 'settings.appearance.showUnreadAccountBadge', keywords: 'count' },
]
