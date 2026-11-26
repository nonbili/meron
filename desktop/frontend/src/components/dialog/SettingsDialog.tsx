import { useEffect, useRef, type ReactNode, type RefObject } from 'react'
import { useTranslation } from '../../lib/i18n'
import { Dialog } from './Dialog'
import type { LucideIcon } from 'lucide-react'
import {
  CalendarDays,
  X,
  SlidersHorizontal,
  Plus,
  Columns3,
  Lock,
  Pause,
  BellOff,
  MessagesSquare,
  ListFilter,
  Server,
  SquarePen,
} from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { ui$, type SetupMode } from '../../states/ui'
import { accounts$, addLocalAccount, isLocalAccountId } from '../../states/accounts'
import { settings$, type KanbanBoard } from '../../states/settings'
import { createKanbanBoard } from '../../states/kanban'
import type { Account } from '../../types'
import { AccountAvatar } from '../avatar/AccountAvatar'
import { IconButton } from '../button/IconButton'
import { BoardPanel } from './BoardSettingsPanel'
import { McpSettingsPanel } from './McpSettingsPanel'
import { EncryptionSettingsPage } from '../crypto/EncryptionSettingsPage'
import { RulesPanel } from './RulesPanel'
import { AccountPanel, accountMeta, isRssAccount } from './AccountPanel'
import { SettingsGeneralSection } from './SettingsGeneralSection'
import { SettingsCalendarsTasksSection } from './SettingsCalendarsTasksSection'
import { SettingsMessagesSection } from './SettingsMessagesSection'
import { SettingsComposerSection } from './SettingsComposerSection'

export { AccountPanel } from './AccountPanel'

// General uses the empty selection; Messages, Composer, Rules and MCP have ids of
// their own. Account and board IDs share the selection state with these
// settings sections.
type SectionId = 'general' | 'messages' | 'composer' | 'rules'
const SECTIONS: { id: SectionId; labelKey: string; icon: LucideIcon }[] = [
  { id: 'general', labelKey: 'settings.sections.general', icon: SlidersHorizontal },
  { id: 'messages', labelKey: 'settings.sections.messages', icon: MessagesSquare },
  { id: 'composer', labelKey: 'settings.sections.composer', icon: SquarePen },
  { id: 'rules', labelKey: 'rules.title', icon: ListFilter },
]
const PAGE_IDS = new Set<string>(['messages', 'composer', 'rules', 'mcp', 'encryption', 'calendars'])

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
  const activeKey: string = selectedAccount || selectedBoard || PAGE_IDS.has(selected) ? selected : 'general'

  const mailAccounts = accounts.filter((acc) => !isRssAccount(acc) && !isLocalAccountId(acc.id))
  const feedAccounts = accounts.filter(isRssAccount)
  const localAccounts = accounts.filter((acc) => isLocalAccountId(acc.id))
  const contentRef = useRef<HTMLDivElement>(null)
  useRevealSettingRow(contentRef, activeKey)

  // Remember the page shown on close so reopening Settings returns to it (in
  // memory only; a restart starts on General). Saving the displayed page, not
  // the raw id, drops a removed account's id instead of reviving it later.
  const onClose = () => {
    ui$.accountSettingsId.set(activeKey === 'general' ? '' : activeKey)
    ui$.settingsOpen.set(false)
  }

  // Open the add-account dialog layered on top of Settings (Settings stays open
  // underneath, so cancelling or saving returns here).
  const onAddAccount = (mode: SetupMode) => {
    ui$.setupMode.set(mode)
    ui$.setupOpen.set(true)
  }

  return (
    <Dialog
      onClose={() => {
        if (!ui$.setupOpen.peek()) onClose()
      }}
      aria-label={t('settings.label')}
      backdropClassName="bg-black/35"
      className="border-border/80 max-w-4xl h-[760px] max-h-[92vh] shadow-black/20 dark:shadow-black/45 flex flex-col overflow-hidden"
    >
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
          {SECTIONS.map(({ id, labelKey, icon: Icon }) => (
            <NavItem
              key={id}
              active={activeKey === id}
              onClick={id === 'general' ? selectGeneral : () => selectAccount(id)}
            >
              <Icon size={15} className="shrink-0" />
              <span className="truncate">{t(labelKey)}</span>
            </NavItem>
          ))}

          <NavItem active={activeKey === 'calendars'} onClick={() => selectAccount('calendars')}>
            <CalendarDays size={15} className="shrink-0" />
            <span>{t('settings.calendars.nav')}</span>
          </NavItem>
          <NavItem active={activeKey === 'encryption'} onClick={() => selectAccount('encryption')}>
            <Lock size={15} className="shrink-0" />
            <span>{t('crypto.peers.nav')}</span>
          </NavItem>
          <NavItem active={activeKey === 'mcp'} onClick={() => selectAccount('mcp')}>
            <Server size={15} className="shrink-0" />
            <span>{t('mcp.title')}</span>
          </NavItem>
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
          <AccountGroup
            label={t('settings.sections.localFolders')}
            accounts={localAccounts}
            activeKey={activeKey}
            onSelect={selectAccount}
            // There is one set of local folders: once added, nothing to add.
            onAdd={localAccounts.length === 0 ? () => void addLocalAccount() : undefined}
            emptyLabel={t('settings.sections.noLocalFolders')}
          />
        </nav>

        {/* Content */}
        <div ref={contentRef} className="flex-1 min-w-0 overflow-y-auto bg-chats p-6">
          {selectedAccount ? (
            <AccountPanel account={selectedAccount} />
          ) : selectedBoard ? (
            <BoardPanel board={selectedBoard} />
          ) : selected === 'mcp' ? (
            <McpSettingsPanel />
          ) : selected === 'encryption' ? (
            <EncryptionSettingsPage />
          ) : selected === 'calendars' ? (
            <SettingsCalendarsTasksSection />
          ) : selected === 'messages' ? (
            <SettingsMessagesSection />
          ) : selected === 'composer' ? (
            <SettingsComposerSection />
          ) : selected === 'rules' ? (
            <RulesPanel />
          ) : (
            <SettingsGeneralSection />
          )}
        </div>
      </div>
    </Dialog>
  )
}

// Brings the row asked for by openSettingsAt (e.g. from the command palette)
// into view once its page is showing: scrolls to it, flashes it, and focuses
// its control so Space or Enter works it straight away. A row the page doesn't
// show right now (say, a platform-only one) leaves just the page open.
function useRevealSettingRow(contentRef: RefObject<HTMLDivElement | null>, activeKey: string) {
  const reveal = useValue(ui$.settingsReveal)

  useEffect(() => {
    if (!reveal) return
    // After the page and the dialog's own initial focus have settled.
    const frame = requestAnimationFrame(() => {
      ui$.settingsReveal.set('')
      const row = contentRef.current?.querySelector<HTMLElement>(`[data-setting-title="${CSS.escape(reveal)}"]`)
      if (!row) return
      row.scrollIntoView({ block: 'center' })
      // Removed when the flash ends, so the next reveal of this row replays it.
      row.classList.add('setting-reveal')
      row.addEventListener('animationend', () => row.classList.remove('setting-reveal'), { once: true })
      // The row's own control, not its info tip: a field before any button
      // beside it (a number row's Reset), then a switch or segment. A group
      // whose content isn't all SettingRows falls back to its editor (the
      // signature's, past its toolbar), else its first real control.
      const control = [
        '[data-setting-control] :is(input, select, textarea)',
        '[data-setting-control] :is(button, [tabindex]:not([tabindex="-1"]))',
        '[contenteditable="true"]',
        'button, input, select, textarea',
      ].reduce<HTMLElement | null>((found, selector) => found ?? row.querySelector<HTMLElement>(selector), null)
      control?.focus({ preventScroll: true })
    })
    return () => cancelAnimationFrame(frame)
  }, [reveal, activeKey, contentRef])
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
  onAdd?: () => void
  emptyLabel: string
}) {
  const { t } = useTranslation()
  return (
    <>
      <div className="mt-5 mb-1.5 flex min-h-6 items-center justify-between px-3">
        <span className="text-[0.6875rem] font-semibold text-secondary">{label}</span>
        {onAdd && (
          <button
            onClick={onAdd}
            title={t('accounts.actions.addAccount')}
            className="flex h-6 w-6 items-center justify-center rounded-lg text-secondary hover:text-accent hover:bg-accent/10 cursor-pointer transition-colors"
          >
            <Plus size={13} />
          </button>
        )}
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
      <AccountAvatar
        accountId={account.id}
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
