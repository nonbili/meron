import { useEffect, useRef, useState, type MouseEvent, type ReactNode } from 'react'
import { useTranslation } from '../../lib/i18n'
import { useEscapeKey } from '../../lib/useEscapeKey'
import type { LucideIcon } from 'lucide-react'
import {
  X,
  Upload,
  Download,
  SlidersHorizontal,
  Image as ImageIcon,
  Send,
  Inbox,
  Plus,
  Trash2,
  Camera,
  Columns3,
  Lock,
  Globe,
  KeyRound,
  SpellCheck,
  Pause,
  BellOff,
  ScrollText,
  RefreshCw,
  MessagesSquare,
  Keyboard,
  Archive,
  Server,
} from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { importOpml, exportOpml } from '../../states/feeds'
import { exportBackup, importBackup, backupErrorMessage, isWrongPassphrase } from '../../states/backup'
import { BackupPassphraseDialog, type BackupPassphraseMode } from './BackupPassphraseDialog'
import { showToast, ui$, type SetupMode } from '../../states/ui'
import { accounts$, deleteAccount } from '../../states/accounts'
import {
  settings$,
  clampKanbanColumnWidth,
  KANBAN_COLUMN_MAX_WIDTH,
  KANBAN_COLUMN_MIN_WIDTH,
  sendShortcutLabel,
  setUnifiedInboxSideNavVisible,
  type ConversationLayout,
  type KanbanBoard,
  type SendShortcut,
} from '../../states/settings'
import { createKanbanBoard } from '../../states/kanban'
import { update$ } from '../../states/update'
import type { Account } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { IconButton } from '../button/IconButton'
import { NumberRow, SegmentedRow, SettingRow, SettingsGroup, ToggleRow, SelectRow } from './AccountSettingsRows'
import { supportedI18nLanguages, languageNativeNames, type SupportedI18nLanguage } from '../../lib/i18n'
import { ThemeSettingsSection } from './ThemeSettingsSection'
import { FontSettingsSection } from './FontSettingsSection'
import { AccountProxyCard, ProxySettingsSection } from './ProxySettingsCard'
import { AccountSignatureCard, SignatureSettingsSection } from './SignatureSettingsCard'
import { AccountProfileGroup } from './AccountProfileGroup'
import { useAccountAvatar } from './useAccountAvatar'
import { AccountAliasesCard } from './AccountAliasesCard'
import { AccountTogglesSection } from './AccountTogglesSection'
import { AccountWallpaperCard } from './AccountWallpaperCard'
import { AvatarCropDialog } from './AvatarCropDialog'
import { BoardPanel } from './BoardSettingsPanel'
import { RemoteSendersDialog } from './RemoteSendersDialog'
import { pickImageFile } from '../../lib/nativeFilePicker'
import { invoke } from '../../lib/bridge'

// The single non-account section. Account ids never collide with "general", so
// one `selected` string can address either.
const SECTIONS: { id: 'general'; label: string; icon: LucideIcon }[] = [
  { id: 'general', label: 'General', icon: SlidersHorizontal },
]

const SEND_SHORTCUT_OPTIONS: { value: SendShortcut; label: string }[] = [
  { value: 'enter', label: sendShortcutLabel('enter') },
  { value: 'mod_enter', label: sendShortcutLabel('mod_enter') },
]

const CONVERSATION_LAYOUT_OPTIONS = (
  t: ReturnType<typeof useTranslation>['t'],
): { value: ConversationLayout; label: string }[] => [
  { value: 'chat', label: t('settings.appearance.conversationLayoutChat') },
  { value: 'traditional', label: t('settings.appearance.conversationLayoutTraditional') },
]

function isRssAccount(account: Account) {
  return account.provider === 'rss' || account.auth_type === 'rss'
}

// The account's servers as one line, so the settings row shows what is
// configured without opening the editor. Mirrors the security labels the setup
// form offers (TLS / STARTTLS / None).
function serverSummary(account: Account, t: ReturnType<typeof useTranslation>['t']) {
  // TLS/STARTTLS are protocol names and stay verbatim; only "none" is prose.
  const security = (tls?: boolean, starttls?: boolean) =>
    starttls ? 'STARTTLS' : tls === false ? t('accounts.security.none') : 'TLS'
  const imap = `${account.imap_host || '—'}:${account.imap_port || 993} (${security(account.tls, account.starttls)})`
  const smtp = `${account.smtp_host || '—'}:${account.smtp_port || 465} (${security(account.smtp_tls, account.smtp_starttls)})`
  return `IMAP ${imap} · SMTP ${smtp}`
}

function reconnectMode(account: Account): SetupMode {
  if (account.auth_type === 'outlook_oauth' || account.provider === 'outlook') return 'outlook'
  if (account.auth_type === 'gmail_oauth' || account.provider === 'gmail') return 'gmail'
  return 'custom'
}

function accountMeta(account: Account, t: ReturnType<typeof useTranslation>['t']) {
  const isRSS = isRssAccount(account)
  const displayName = account.display_name || (isRSS ? t('accounts.rssFeeds') : account.email.split('@')[0])
  const subtitle = isRSS ? t('accounts.rssAtomFeeds') : account.email
  return { isRSS, displayName, subtitle }
}

export function SettingsDialog() {
  const { t } = useTranslation()
  const accounts = useValue(accounts$)
  const boards = useValue(settings$.kanbanBoards)
  // The selected account or board id ("" = General; account, board, and section
  // ids never collide). Kept in shared state so flows outside the dialog (e.g.
  // saving a new account, the board context menu) can navigate the panel.
  const selected = useValue(ui$.accountSettingsId)
  const selectGeneral = () => ui$.accountSettingsId.set('')
  const selectAccount = (id: string) => ui$.accountSettingsId.set(id)

  const selectedAccount = accounts.find((acc) => acc.id === selected)
  const selectedBoard = !selectedAccount ? boards.find((board) => board.id === selected) : undefined
  // A removed account/board (or a stale id) falls back to General.
  const activeKey: string = selectedAccount || selectedBoard ? selected : 'general'

  const mailAccounts = accounts.filter((acc) => !isRssAccount(acc))
  const feedAccounts = accounts.filter(isRssAccount)

  const onClose = () => {
    ui$.accountSettingsId.set('')
    ui$.settingsOpen.set(false)
  }

  const onBackdropMouseDown = (event: MouseEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget || ui$.setupOpen.peek()) return
    onClose()
  }

  // Esc closes the dialog, unless the add-account dialog is layered on top (it
  // owns the keystroke then).
  useEscapeKey(() => {
    if (ui$.setupOpen.peek()) return
    onClose()
  })

  // Open the add-account dialog layered on top of Settings (Settings stays open
  // underneath, so cancelling or saving returns here).
  const onAddAccount = (mode: SetupMode) => {
    ui$.setupMode.set(mode)
    ui$.setupOpen.set(true)
  }

  return (
    <div
      onMouseDown={onBackdropMouseDown}
      className="fixed inset-0 flex items-center justify-center bg-black/35 dark:bg-black/60 backdrop-blur-[3px] z-50 p-4 select-none animate-fade-in"
    >
      <div className="bg-chats border border-border/80 text-primary max-w-4xl w-full h-[620px] max-h-[90vh] rounded-3xl shadow-2xl shadow-black/20 dark:shadow-black/45 animate-slide-up flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between gap-4 px-6 py-4.5 border-b border-border/60 shrink-0 bg-chats/95">
          <div className="min-w-0">
            <h2 className="text-base font-bold tracking-tight leading-tight">{t('settings.label')}</h2>
          </div>
          <IconButton icon={X} iconSize={16} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>

        {/* Body: nav rail + content */}
        <div className="flex flex-1 min-h-0">
          {/* Nav rail */}
          <nav className="w-56 shrink-0 border-r border-border/60 p-3.5 flex flex-col gap-1 bg-raised/70 overflow-y-auto">
            {SECTIONS.map(({ id, label, icon: Icon }) => (
              <NavItem key={id} active={activeKey === id} onClick={selectGeneral}>
                <Icon size={15} className="shrink-0" />
                <span className="truncate">{id === 'general' ? t('settings.sections.general') : label}</span>
              </NavItem>
            ))}

            <BoardGroup boards={boards} activeKey={activeKey} onSelect={selectAccount} />
            <AccountGroup
              label={t('settings.sections.mailAccounts')}
              accounts={mailAccounts}
              activeKey={activeKey}
              onSelect={selectAccount}
              onAdd={() => onAddAccount('gmail')}
              emptyLabel={t('settings.sections.noMailAccounts')}
            />
            <AccountGroup
              label={t('settings.sections.feedAccounts')}
              accounts={feedAccounts}
              activeKey={activeKey}
              onSelect={selectAccount}
              onAdd={() => onAddAccount('rss')}
              emptyLabel={t('settings.sections.noFeedAccounts')}
            />
          </nav>

          {/* Content */}
          <div className="flex-1 min-w-0 overflow-y-auto bg-chats p-6">
            {selectedAccount ? (
              <AccountPanel account={selectedAccount} />
            ) : selectedBoard ? (
              <BoardPanel board={selectedBoard} />
            ) : (
              <GeneralSection />
            )}
          </div>
        </div>
      </div>
    </div>
  )
}

function NavItem({
  active,
  onClick,
  title,
  children,
}: {
  active: boolean
  onClick: () => void
  title?: string
  children: ReactNode
}) {
  const { t } = useTranslation()
  return (
    <button
      onClick={onClick}
      title={title}
      className={`flex min-h-9 items-center gap-2.5 rounded-xl border px-3 py-2 text-xs font-semibold transition-colors cursor-pointer text-left ${
        active
          ? 'bg-accent/10 border-accent/20 text-accent shadow-sm'
          : 'border-transparent text-secondary hover:text-primary hover:bg-hover/80'
      }`}
    >
      {children}
    </button>
  )
}

function AccountGroup({
  label,
  accounts,
  activeKey,
  onSelect,
  onAdd,
  emptyLabel,
}: {
  label: string
  accounts: Account[]
  activeKey: string
  onSelect: (id: string) => void
  onAdd: () => void
  emptyLabel: string
}) {
  const { t } = useTranslation()
  return (
    <>
      <div className="mt-5 mb-1.5 flex items-center justify-between px-3">
        <span className="text-[0.6875rem] font-semibold text-secondary">{label}</span>
        <button
          onClick={onAdd}
          title={t('accounts.actions.addAccount')}
          className="flex h-6 w-6 items-center justify-center rounded-lg text-secondary hover:text-accent hover:bg-accent/10 cursor-pointer transition-colors"
        >
          <Plus size={13} />
        </button>
      </div>
      {accounts.length === 0 ? (
        <p className="px-3 py-1 text-[0.65625rem] text-secondary font-medium">{emptyLabel}</p>
      ) : (
        accounts.map((account) => {
          const { displayName, subtitle } = accountMeta(account, t)
          return (
            <NavItem
              key={account.id}
              active={activeKey === account.id}
              onClick={() => onSelect(account.id)}
              title={subtitle}
            >
              <AccountNavAvatar account={account} displayName={displayName} />
              <span className="truncate">{displayName}</span>
            </NavItem>
          )
        })
      )}
    </>
  )
}

function AccountNavAvatar({ account, displayName }: { account: Account; displayName: string }) {
  const isPaused = account.paused ?? false
  const isMuted = account.muted ?? false

  return (
    <span className="relative shrink-0">
      <Avatar
        name={displayName}
        src={account.avatar_url}
        size={20}
        className={`!rounded-md transition-all ${isPaused ? 'grayscale opacity-40' : ''}`}
      />
      {(isPaused || isMuted) && (
        <span className="absolute -bottom-1 -right-1 flex h-3.5 w-3.5 items-center justify-center rounded-full bg-black/60 text-white/80 ring-2 ring-raised">
          {isPaused ? <Pause size={7} className="fill-current" /> : <BellOff size={7} />}
        </span>
      )}
    </span>
  )
}

function BoardGroup({
  boards,
  activeKey,
  onSelect,
}: {
  boards: KanbanBoard[]
  activeKey: string
  onSelect: (id: string) => void
}) {
  const { t } = useTranslation()
  return (
    <>
      <div className="mt-5 mb-1.5 flex items-center justify-between px-3">
        <span className="text-[0.6875rem] font-semibold text-secondary">{t('settings.sections.kanbanBoards')}</span>
        <button
          onClick={() => onSelect(createKanbanBoard())}
          title={t('kanban.actions.addBoard')}
          className="flex h-6 w-6 items-center justify-center rounded-lg text-secondary hover:text-accent hover:bg-accent/10 cursor-pointer transition-colors"
        >
          <Plus size={13} />
        </button>
      </div>
      {boards.length === 0 ? (
        <p className="px-3 py-1 text-[0.65625rem] text-secondary font-medium">{t('settings.sections.noBoards')}</p>
      ) : (
        boards.map((board) => (
          <NavItem key={board.id} active={activeKey === board.id} onClick={() => onSelect(board.id)}>
            {board.avatarUrl ? (
              <img src={board.avatarUrl} alt="" className="h-5 w-5 shrink-0 rounded-md object-cover" />
            ) : (
              <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-md bg-accent/10 text-accent">
                <Columns3 size={12} />
              </span>
            )}
            <span className="truncate">{board.name}</span>
          </NavItem>
        ))
      )}
    </>
  )
}

function GeneralSection() {
  const { t } = useTranslation()
  const [remoteSendersOpen, setRemoteSendersOpen] = useState(false)
  const remoteImageSenders = useValue(settings$.remoteImageSenders)
  const showRealAvatars = useValue(settings$.showRealAvatars)
  const showUnreadAccountBadge = useValue(settings$.showUnreadAccountBadge)
  const conversationLayout = useValue(settings$.conversationLayout)
  const sendShortcut = useValue(settings$.sendShortcut)
  const spellCheck = useValue(settings$.spellCheck)
  const showUnifiedInbox = useValue(settings$.showUnifiedInboxInSideNav)
  const kanbanColumnWidth = useValue(settings$.kanbanColumnWidth)
  const kanbanLockScroll = useValue(settings$.kanbanLockScroll)
  const language = useValue(settings$.language)

  return (
    <div className="flex flex-col gap-4">
      <SettingsGroup title={t('settings.pages.appearance')}>
        <ThemeSettingsSection />
        <SegmentedRow
          icon={<MessagesSquare size={15} />}
          title={t('settings.appearance.conversationLayout')}
          hint={t('settings.appearance.conversationLayoutHint')}
          value={conversationLayout}
          options={CONVERSATION_LAYOUT_OPTIONS(t)}
          onChange={(value) => settings$.conversationLayout.set(value)}
        />
        <ToggleRow
          icon={<ImageIcon size={15} />}
          title={t('settings.appearance.showSenderImages')}
          hint={t('settings.appearance.showSenderImagesHint')}
          checked={showRealAvatars}
          onChange={() => settings$.showRealAvatars.set(!showRealAvatars)}
        />
        <ToggleRow
          icon={<Inbox size={15} />}
          title={t('settings.appearance.showUnreadAccountBadge')}
          hint={t('settings.appearance.showUnreadAccountBadgeHint')}
          checked={showUnreadAccountBadge}
          onChange={() => settings$.showUnreadAccountBadge.set(!showUnreadAccountBadge)}
        />
      </SettingsGroup>

      <SettingsGroup title={t('settings.sections.typography')}>
        <FontSettingsSection />
      </SettingsGroup>

      <SettingsGroup title={t('settings.language.label')}>
        <SelectRow
          icon={<Globe size={15} />}
          title={t('settings.language.label')}
          hint={t('settings.language.hint')}
          value={language || ''}
          options={[
            { value: '', label: t('settings.language.system') },
            ...supportedI18nLanguages.map((lang) => ({
              value: lang,
              label: languageNativeNames[lang],
            })),
          ]}
          onChange={(value) => {
            settings$.language.set(value === '' ? null : (value as SupportedI18nLanguage))
          }}
        />
      </SettingsGroup>

      <SettingsGroup title={t('settings.sections.sideNav')}>
        <ToggleRow
          icon={<Inbox size={15} />}
          title={t('settings.sideNav.showUnifiedInbox')}
          checked={showUnifiedInbox}
          onChange={() => setUnifiedInboxSideNavVisible(!showUnifiedInbox)}
        />
      </SettingsGroup>

      <SettingsGroup title={t('settings.sections.privacy')}>
        <SettingRow
          title={t('settings.privacy.remoteSenders')}
          hint={t('settings.privacy.remoteSendersHint')}
          control={
            <button
              onClick={() => setRemoteSendersOpen(true)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
            >
              {remoteImageSenders.length > 0
                ? t('settings.privacy.remoteSendersCount', { count: remoteImageSenders.length })
                : t('settings.privacy.remoteSendersNone')}
            </button>
          }
        />
      </SettingsGroup>
      {remoteSendersOpen && <RemoteSendersDialog onClose={() => setRemoteSendersOpen(false)} />}

      <ProxySettingsSection />

      <SettingsGroup title={t('settings.sections.kanban')}>
        <NumberRow
          icon={<Columns3 size={15} />}
          title={t('settings.kanban.columnWidth')}
          value={String(kanbanColumnWidth)}
          min={KANBAN_COLUMN_MIN_WIDTH}
          max={KANBAN_COLUMN_MAX_WIDTH}
          step={10}
          suffix="px"
          onChange={(value) => {
            const width = Number(value)
            if (Number.isFinite(width)) settings$.kanbanColumnWidth.set(clampKanbanColumnWidth(width))
          }}
        />
        <ToggleRow
          icon={<Lock size={15} />}
          title={t('settings.kanban.lockScroll')}
          hint={t('settings.kanban.lockScrollHint')}
          checked={kanbanLockScroll}
          onChange={() => settings$.kanbanLockScroll.set(!kanbanLockScroll)}
        />
      </SettingsGroup>

      <SettingsGroup title={t('settings.sections.composer')}>
        <ToggleRow
          icon={<SpellCheck size={15} />}
          title={t('settings.composer.spellCheck')}
          hint={t('settings.composer.spellCheckHint')}
          checked={spellCheck}
          onChange={() => settings$.spellCheck.set(!spellCheck)}
        />
        <SegmentedRow
          icon={<Send size={15} />}
          title={t('settings.composer.sendMessageWith')}
          hint={
            sendShortcut === 'enter'
              ? t('settings.composer.sendShortcutEnterHint')
              : t('settings.composer.sendShortcutModHint', { shortcut: sendShortcutLabel('mod_enter') })
          }
          value={sendShortcut}
          options={SEND_SHORTCUT_OPTIONS}
          onChange={(value) => settings$.sendShortcut.set(value)}
        />
      </SettingsGroup>

      <SignatureSettingsSection />

      <SettingsGroup title={t('shortcuts.title')}>
        <SettingRow
          title={t('shortcuts.title')}
          hint={t('shortcuts.customizeHint')}
          control={
            <button
              onClick={() => ui$.shortcutsOpen.set(true)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
            >
              <Keyboard size={12} />
              {t('shortcuts.customize')}
            </button>
          }
        />
      </SettingsGroup>

      <UpdatesGroup />
      <BackupGroup />
      <StorageGroup />
      <LogsGroup />
    </div>
  )
}

// Backup / restore of the app's configuration. Cached mail is not included, so
// a restored account re-syncs rather than arriving with its history.
//
// Both buttons hand off to a passphrase dialog: exporting to offer encryption,
// restoring only when the chosen file turns out to be encrypted (which the
// bridge reports back with the path, so the file dialog isn't shown twice).
function BackupGroup() {
  const { t } = useTranslation()
  const [prompt, setPrompt] = useState<BackupPassphraseMode | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  // The file already chosen for a restore that came back needing a passphrase.
  const [pendingPath, setPendingPath] = useState('')

  const closePrompt = () => {
    setPrompt(null)
    setError('')
    setPendingPath('')
  }

  const runExport = async (passphrase: string, includeSecrets: boolean) => {
    setBusy(true)
    setError('')
    try {
      const saved = await exportBackup(includeSecrets, passphrase)
      closePrompt()
      if (saved) showToast(t('settings.backup.exported'), 'success')
    } catch (err) {
      setError(backupErrorMessage(err, t('settings.backup.exportFailed')))
    } finally {
      setBusy(false)
    }
  }

  // `path` is empty on the first attempt (the bridge opens a file dialog) and
  // set on the retry after a passphrase prompt.
  const runImport = async (path: string, passphrase: string) => {
    setBusy(true)
    setError('')
    try {
      const outcome = await importBackup(path, passphrase)
      if (outcome.status === 'cancelled') {
        closePrompt()
        return
      }
      if (outcome.status === 'needs-passphrase') {
        setPendingPath(outcome.path)
        setPrompt('restore')
        return
      }
      closePrompt()
      const { accounts, skipped } = outcome.summary
      if (accounts === 0 && skipped > 0) {
        showToast(t('settings.backup.restoredNothingNew'), 'success')
      } else {
        showToast(t('settings.backup.restored', { count: accounts }), 'success')
      }
    } catch (err) {
      const message = backupErrorMessage(err, t('settings.backup.restoreFailed'))
      // A wrong passphrase keeps the prompt open so the user can retype it;
      // anything else is a real failure and closes it.
      if (isWrongPassphrase(message) && pendingPath) {
        setError(t('settings.backup.wrongPassphrase'))
      } else {
        closePrompt()
        showToast(message, 'error')
      }
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      <SettingsGroup title={t('settings.sections.backup')}>
        <SettingRow
          title={t('settings.backup.fileTitle')}
          hint={t('settings.backup.fileHint')}
          control={
            <div className="flex items-center gap-2">
              <button
                onClick={() => runImport('', '')}
                disabled={busy}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Upload size={12} />
                {t('settings.backup.restoreAction')}
              </button>
              <button
                onClick={() => {
                  setError('')
                  setPrompt('export')
                }}
                disabled={busy}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Archive size={12} />
                {t('common.export')}
              </button>
            </div>
          }
        />
      </SettingsGroup>

      {prompt && (
        <BackupPassphraseDialog
          mode={prompt}
          busy={busy}
          error={error}
          onCancel={closePrompt}
          onSubmit={(passphrase, includeSecrets) =>
            prompt === 'export' ? runExport(passphrase, includeSecrets) : runImport(pendingPath, passphrase)
          }
        />
      )}
    </>
  )
}

// Only meaningful where the app can actually replace itself; store-managed and
// dev builds get no toggle at all rather than one that does nothing.
function UpdatesGroup() {
  const { t } = useTranslation()
  const status = useValue(update$.status)
  const autoUpdateCheck = useValue(settings$.autoUpdateCheck)

  if (!status.supported) return null

  return (
    <SettingsGroup title={t('settings.sections.updates')}>
      <ToggleRow
        icon={<RefreshCw size={15} />}
        title={t('settings.updates.autoCheck')}
        hint={t('settings.updates.autoCheckHint')}
        checked={autoUpdateCheck}
        onChange={() => settings$.autoUpdateCheck.set(!autoUpdateCheck)}
      />
    </SettingsGroup>
  )
}

type StorageUsage = { cacheBytes: number; dbBytes: number }

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const value = bytes / 1024 ** i
  return `${value >= 100 || i === 0 ? Math.round(value) : value.toFixed(1)} ${units[i]}`
}

function StorageGroup() {
  const { t } = useTranslation()
  const [usage, setUsage] = useState<StorageUsage | null>(null)
  const [clearing, setClearing] = useState(false)
  // Two-step guard: the first click arms "Confirm?", the second actually clears.
  // Prevents wiping the cache from a stray click. Auto-disarms after a few seconds.
  const [confirming, setConfirming] = useState(false)

  useEffect(() => {
    let alive = true
    invoke<StorageUsage>('storage.usage')
      .then((u) => {
        if (alive) setUsage(u)
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [])

  useEffect(() => {
    if (!confirming) return
    const id = setTimeout(() => setConfirming(false), 4000)
    return () => clearTimeout(id)
  }, [confirming])

  const clearCache = async () => {
    if (!confirming) {
      setConfirming(true)
      return
    }
    setConfirming(false)
    setClearing(true)
    try {
      const u = await invoke<StorageUsage>('storage.clearCache')
      setUsage(u)
      showToast(t('settings.storage.clearedToast'), 'success')
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), 'error')
    } finally {
      setClearing(false)
    }
  }

  return (
    <SettingsGroup title={t('settings.sections.storage')}>
      <SettingRow
        title={t('settings.storage.usageTitle')}
        hint={t('settings.storage.usageHint')}
        control={
          <span className="text-xs text-secondary tabular-nums">
            {usage
              ? `${t('settings.storage.cacheLabel')} ${formatBytes(usage.cacheBytes)} · ${t('settings.storage.databaseLabel')} ${formatBytes(usage.dbBytes)}`
              : '…'}
          </span>
        }
      />
      <SettingRow
        title={t('settings.storage.clearTitle')}
        hint={t('settings.storage.clearHint')}
        control={
          <button
            onClick={clearCache}
            disabled={clearing || (usage?.cacheBytes ?? 0) === 0}
            className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl font-bold text-[0.625rem] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${
              confirming ? 'bg-red-500 hover:bg-red-600 text-white' : 'bg-hover hover:bg-active text-primary'
            }`}
          >
            <Trash2 size={12} />
            {confirming ? t('settings.storage.clearConfirm') : t('settings.storage.clearButton')}
          </button>
        }
      />
    </SettingsGroup>
  )
}

function LogsGroup() {
  const { t } = useTranslation()
  const [viewerOpen, setViewerOpen] = useState(false)
  return (
    <SettingsGroup title={t('settings.sections.logs')}>
      <SettingRow
        title={t('settings.sections.logs')}
        hint={t('settings.syncDiagnosticLogHint')}
        control={
          <button
            onClick={() => setViewerOpen(true)}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
          >
            <ScrollText size={12} />
            {t('settings.viewSyncLog')}
          </button>
        }
      />
      {viewerOpen && <LogViewerDialog onClose={() => setViewerOpen(false)} />}
    </SettingsGroup>
  )
}

// In-app viewer for the local app log, so the user can inspect what would be
// shared before exporting it (export lives in the dialog header).
function LogViewerDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation()
  const [log, setLog] = useState<string | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  // The newest entries are at the end; start there.
  useEffect(() => {
    const el = scrollRef.current
    if (el && log) el.scrollTop = el.scrollHeight
  }, [log])

  useEffect(() => {
    let alive = true
    invoke<{ log: string }>('log.read')
      .then((res) => {
        if (alive) setLog(res?.log ?? '')
      })
      .catch((error) => {
        showToast(error instanceof Error ? error.message : String(error), 'error')
        if (alive) setLog('')
      })
    return () => {
      alive = false
    }
  }, [])

  // Layered above Settings; useEscapeKey hands Esc to the topmost layer only,
  // so Settings stays open underneath.
  useEscapeKey(onClose)

  const exportLog = async () => {
    try {
      const res = await invoke<{ saved: boolean; path?: string }>('log.export')
      if (res?.saved) showToast(t('settings.toast.logExported'), 'success')
    } catch (error) {
      showToast(error instanceof Error ? error.message : String(error), 'error')
    }
  }

  return (
    <div
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose()
      }}
      className="fixed inset-0 flex items-center justify-center bg-black/35 dark:bg-black/60 backdrop-blur-[3px] z-50 p-4 select-none animate-fade-in"
    >
      <div className="bg-chats border border-border/80 text-primary max-w-3xl w-full h-[560px] max-h-[85vh] rounded-3xl shadow-2xl shadow-black/20 dark:shadow-black/45 animate-slide-up flex flex-col overflow-hidden">
        <div className="flex items-center justify-between gap-4 px-6 py-4.5 border-b border-border/60 shrink-0 bg-chats/95">
          <h2 className="text-base font-bold tracking-tight leading-tight">{t('settings.viewSyncLog')}</h2>
          <div className="flex items-center gap-2">
            <button
              onClick={() => void exportLog()}
              disabled={!log}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Download size={12} />
              {t('common.export')}
            </button>
            <IconButton icon={X} iconSize={16} label={t('buttons.close')} size="sm" onClick={onClose} />
          </div>
        </div>
        <div ref={scrollRef} className="flex-1 min-h-0 overflow-y-auto p-4 select-text">
          {log === null ? (
            <p className="text-xs text-secondary">{t('common.loading')}</p>
          ) : log === '' ? (
            <p className="text-xs text-secondary">{t('settings.syncLogEmpty')}</p>
          ) : (
            <pre className="whitespace-pre-wrap break-all font-mono text-[0.6875rem] leading-4 text-primary">{log}</pre>
          )}
        </div>
      </div>
    </div>
  )
}

function OpmlGroup({ account }: { account: string }) {
  const { t } = useTranslation()
  return (
    <SettingsGroup title={t('settings.sections.subscriptions')}>
      <SettingRow
        title={t('settings.feeds.opmlFile')}
        hint={t('settings.feeds.opmlHint')}
        control={
          <div className="flex items-center gap-2">
            <button
              onClick={() => importOpml(account)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
            >
              <Upload size={12} />
              {t('common.import')}
            </button>
            <button
              onClick={() => exportOpml(account)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
            >
              <Download size={12} />
              {t('common.export')}
            </button>
          </div>
        }
      />
    </SettingsGroup>
  )
}

function AccountPanel({ account }: { account: Account }) {
  const { t } = useTranslation()
  const { isRSS, displayName, subtitle } = accountMeta(account, t)
  const [avatarFile, setAvatarFile] = useState<File | null>(null)
  const { avatarBusy, persistAvatarFile } = useAccountAvatar(account.id)

  const reconnectAccount = () => {
    ui$.reconnectAccountId.set(account.id)
    ui$.setupMode.set(reconnectMode(account))
    ui$.setupOpen.set(true)
  }

  const pickAvatarFile = async () => {
    try {
      setAvatarFile(await pickImageFile(t('settings.account.chooseAvatarImage')))
    } catch (error) {
      showToast(error instanceof Error ? error.message : t('settings.account.chooseAvatarFailed'), 'error')
    }
  }

  const saveCroppedAvatar = async (file: File) => {
    if (await persistAvatarFile(file)) {
      setAvatarFile(null)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-3 min-w-0">
        <button
          type="button"
          title={t('settings.account.changeAvatar')}
          disabled={avatarBusy}
          onClick={() => void pickAvatarFile()}
          className="relative shrink-0 rounded-2xl group disabled:cursor-default cursor-pointer"
        >
          <Avatar name={displayName} src={account.avatar_url} size={40} className="!rounded-2xl" />
          <span className="absolute inset-0 flex items-center justify-center rounded-2xl bg-black/45 opacity-0 group-hover:opacity-100 transition-opacity">
            <Camera size={15} className="text-white" />
          </span>
        </button>
        {avatarFile && (
          <AvatarCropDialog
            file={avatarFile}
            busy={avatarBusy}
            onCancel={() => setAvatarFile(null)}
            onSave={saveCroppedAvatar}
          />
        )}
        <div className="min-w-0 flex-1">
          <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight truncate">{displayName}</h2>
          <p className="text-[0.65625rem] text-secondary mt-0.5 font-medium truncate">{subtitle}</p>
        </div>
      </div>

      {!isRSS && account.auth_type === 'password' && !account.needs_reconnect && (
        <SettingsGroup title={t('settings.account.serverTitle', { defaultValue: 'Server settings' })}>
          <SettingRow
            title={t('settings.account.serverAccount', { defaultValue: 'Incoming and outgoing servers' })}
            hint={serverSummary(account, t)}
            control={
              <button
                type="button"
                onClick={reconnectAccount}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-border text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
              >
                <Server size={12} />
                {t('settings.account.serverEdit', { defaultValue: 'Edit' })}
              </button>
            }
          />
        </SettingsGroup>
      )}

      {account.needs_reconnect && !isRSS && (
        <SettingsGroup title={t('settings.account.reconnectTitle', { defaultValue: 'Reconnect' })}>
          <SettingRow
            title={t('settings.account.reconnectAccount', { defaultValue: 'Reconnect account' })}
            hint={t('settings.account.reconnectHint', {
              defaultValue: 'Restore the missing keychain credential for this account.',
            })}
            control={
              <button
                type="button"
                onClick={reconnectAccount}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-accent hover:bg-accent-hover text-white font-bold text-[0.625rem] cursor-pointer transition-colors"
              >
                <KeyRound size={12} />
                {t('settings.account.reconnectButton', { defaultValue: 'Reconnect' })}
              </button>
            }
          />
        </SettingsGroup>
      )}

      <AccountProfileGroup account={account} isRSS={isRSS} />
      <SettingsGroup title={t('settings.pages.appearance')}>
        <AccountWallpaperCard account={account} />
      </SettingsGroup>
      <AccountTogglesSection account={account} isRSS={isRSS} />
      {!isRSS && <AccountProxyCard account={account} />}
      {!isRSS && <AccountAliasesCard account={account} />}
      {!isRSS && <AccountSignatureCard account={account} />}
      {isRSS && <OpmlGroup account={account.id} />}

      <button
        type="button"
        onClick={() => void deleteAccount(account.id)}
        className="mt-1 self-start flex items-center gap-1.5 rounded-lg px-2 py-1 text-xs font-semibold text-secondary hover:text-rose-500 transition-colors cursor-pointer"
      >
        <Trash2 size={12} />
        {t('settings.account.removeAccount')}
      </button>
    </div>
  )
}
