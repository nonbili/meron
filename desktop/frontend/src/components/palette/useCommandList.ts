import { useMemo } from 'react'
import {
  Archive,
  Columns3,
  FolderOpen,
  Inbox,
  List,
  Mail,
  MailCheck,
  MailOpen,
  Moon,
  Pencil,
  Keyboard,
  Maximize2,
  Plus,
  Reply,
  ReplyAll,
  RefreshCw,
  Search,
  SearchCheck,
  Settings,
  SlidersHorizontal,
  Paperclip,
  Star,
  Sun,
  Trash2,
  Users,
  X,
} from 'lucide-react'
import { createElement } from 'react'
import { openEmlFile } from '../../lib/eml'
import { useTranslation } from '../../lib/i18n'
import { useValue } from '@legendapp/state/react'
import { ui$, closeCommandPalette, focusGlobalSearch, openSettingsAt, showToast } from '../../states/ui'
import { appIconSupported$, selectTheme, settings$ } from '../../states/settings'
import { update$ } from '../../states/update'
import { windowChrome$ } from '../../lib/windowChrome'
import { useChosenThemeIds } from '../../lib/useThemeChoice'
import { accounts$ } from '../../states/accounts'
import { leaveNarrowContacts } from '../../states/contacts'
import { BUILTIN_THEMES } from '../../lib/themes'
import {
  closeKanbanBoard,
  createKanbanBoard,
  kanban$,
  kanbanPaneThreadId,
  markBoardAllRead,
  openMailAccount,
  selectKanbanBoard,
  setGlobalKanbanFilter,
  setGlobalKanbanAttachmentsOnly,
} from '../../states/kanban'
import { thread$ } from '../../states/thread'
import { mail$, syncMail } from '../../states/mail'
import { markAllRead, toggleStarWithUndo, markUnreadWithUndo } from '../../states/mailFlags'
import { archiveThread, deleteThread } from '../../states/mailMoves'
import {
  openComposeTab,
  openReplyInFullEditor,
  closeMessageTab,
  readerTabInFront,
  replyInConversation,
} from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { canReplyAllToThread } from '../../states/composeReply'
import { RAIL_SHORTCUT_IDS, type ShortcutId } from '../../lib/shortcuts'
import type { Command } from './paletteCommands'
import { ACCOUNT_SETTINGS_INDEX, BOARD_SETTINGS_INDEX, SETTINGS_INDEX, accountKind } from './settingsIndex'

// Builds the full, ordered command list from current app state. Each entry's
// `run` closes the palette first, then performs its action. Returns a memoized
// array recomputed only when the underlying selections change.
export function useCommandList(): Command[] {
  const { t } = useTranslation()
  const boards = useValue(settings$.kanbanBoards)
  const activeBoardId = useValue(kanban$.activeBoardId)
  const chosenThemeIds = useChosenThemeIds()
  const customThemes = useValue(settings$.customThemes)
  const filterMode = useValue(ui$.filterMode)
  const kanbanFilterMode = useValue(kanban$.globalFilter)
  const attachmentsOnly = useValue(ui$.attachmentsOnly)
  const kanbanAttachmentsOnly = useValue(kanban$.globalAttachmentsOnly)
  const selectedAccount = useValue(ui$.selectedAccount)
  const selectedFolder = useValue(ui$.selectedFolder)
  const selectedThread = useValue(ui$.selectedThread)
  const accounts = useValue(accounts$)
  const folders = useValue(mail$.folders)
  const activeTab = useValue(compose$.activeTab)
  const replyAll = useValue(canReplyAllToThread)
  // Which settings rows the dialog shows depends on these; read here so the
  // list follows them.
  const calendarEnabled = useValue(settings$.calendarEnabled)
  const conversationLayout = useValue(settings$.conversationLayout)
  const appIconSupported = useValue(appIconSupported$)
  const titlebarSupported = useValue(windowChrome$.supported)
  const updateSupported = useValue(() => update$.status.get().supported)

  return useMemo<Command[]>(() => {
    const run = (fn: () => void) => () => {
      closeCommandPalette()
      fn()
    }
    const icon = (component: typeof Mail) => createElement(component, { size: 15 })
    const activeFilterMode = activeBoardId ? kanbanFilterMode : filterMode
    const setActiveFilterMode = activeBoardId
      ? setGlobalKanbanFilter
      : (mode: typeof filterMode) => ui$.filterMode.set(mode)
    const activeAttachmentsOnly = activeBoardId ? kanbanAttachmentsOnly : attachmentsOnly
    const setActiveAttachmentsOnly = activeBoardId
      ? setGlobalKanbanAttachmentsOnly
      : (on: boolean) => ui$.attachmentsOnly.set(on)
    const railShortcut = (slot: number) => RAIL_SHORTCUT_IDS[slot - 1] as ShortcutId | undefined

    const list: Command[] = [
      {
        id: 'eml.open',
        label: t('eml.open'),
        icon: icon(FolderOpen),
        keywords: 'open eml file email message',
        shortcut: 'eml.open',
        run: run(() => void openEmlFile().catch((error) => showToast(String(error), 'error'))),
      },
      {
        id: 'compose.new',
        label: 'Compose new message',
        icon: icon(Pencil),
        keywords: 'write new email reply',
        shortcut: 'compose.new',
        run: run(() => openComposeTab()),
      },
      {
        id: 'mail.sync',
        label: 'Sync mailbox',
        icon: icon(RefreshCw),
        keywords: 'refresh fetch check',
        shortcut: 'mail.sync',
        run: run(() => void syncMail()),
      },
      {
        id: 'mail.markAllRead',
        label: 'Mark all as read',
        icon: icon(MailCheck),
        keywords: 'clear unread',
        // With a board open, the list behind it is hidden: mark the board instead.
        run: run(() => void (activeBoardId ? markBoardAllRead(activeBoardId) : markAllRead())),
      },
      {
        id: 'search.thread',
        label: 'Search current thread',
        icon: icon(Search),
        keywords: 'find conversation in',
        shortcut: 'search.thread',
        run: run(() => {
          const visible = !!selectedThread && (!activeBoardId || !!kanbanPaneThreadId())
          if (visible) thread$.searchOpen.set(true)
          else focusGlobalSearch()
        }),
      },
      {
        id: 'search.global',
        label: 'Search all messages',
        icon: icon(SearchCheck),
        keywords: 'find global mailbox',
        shortcut: 'search.global',
        run: run(() => focusGlobalSearch()),
      },
      {
        id: 'settings.open',
        label: 'Open settings',
        icon: icon(Settings),
        keywords: 'preferences config',
        shortcut: 'settings.open',
        run: run(() => ui$.settingsOpen.set(true)),
      },
      {
        id: 'account.add',
        label: 'Add account',
        icon: icon(Plus),
        keywords: 'new mailbox connect',
        run: run(() => ui$.setupOpen.set(true)),
      },
      {
        id: 'shortcuts.help',
        label: 'Keyboard shortcuts',
        icon: icon(Keyboard),
        keywords: 'keys cheat sheet help bindings',
        shortcut: 'shortcuts.help',
        run: run(() => ui$.shortcutsOpen.set(true)),
      },
      {
        id: 'view.mail',
        label: 'Go to: Mail',
        icon: icon(List),
        keywords: 'layout list conversation account',
        active: !activeBoardId,
        run: run(() => {
          leaveNarrowContacts()
          closeKanbanBoard()
        }),
      },
      {
        id: 'kanban.create',
        label: 'Create kanban board',
        icon: icon(Plus),
        keywords: 'new board columns',
        run: run(() => createKanbanBoard()),
      },
      ...boards.map((board, boardIndex) => ({
        id: `kanban.${board.id}`,
        label: `Go to: ${board.name}`,
        icon: icon(Columns3),
        keywords: 'kanban board columns',
        shortcut: railShortcut(boardIndex + 2),
        active: activeBoardId === board.id,
        run: run(() => selectKanbanBoard(board.id)),
      })),
      {
        id: 'filter.all',
        label: 'Filter: All mail',
        icon: icon(Mail),
        keywords: 'show everything',
        active: activeFilterMode === 'all',
        run: run(() => setActiveFilterMode('all')),
      },
      {
        id: 'filter.unread',
        label: 'Filter: Unread',
        icon: icon(Inbox),
        active: activeFilterMode === 'unread',
        run: run(() => setActiveFilterMode('unread')),
      },
      {
        id: 'filter.starred',
        label: 'Filter: Starred',
        icon: icon(Star),
        active: activeFilterMode === 'starred',
        run: run(() => setActiveFilterMode('starred')),
      },
      {
        // A toggle on top of the filters above, not a fourth filter.
        id: 'filter.attachments',
        label: activeAttachmentsOnly ? 'Filter: Show all, not only with attachments' : 'Filter: Only with attachments',
        icon: icon(Paperclip),
        keywords: 'files has attachment toggle',
        active: activeAttachmentsOnly,
        run: run(() => setActiveAttachmentsOnly(!activeAttachmentsOnly)),
      },
      ...[...BUILTIN_THEMES, ...customThemes].map((theme) => ({
        id: `theme.${theme.id}`,
        label: `Theme: ${theme.name}`,
        icon: icon(theme.appearance === 'light' ? Sun : Moon),
        active: chosenThemeIds.includes(theme.id),
        run: run(() => selectTheme(theme)),
      })),
    ]

    // A reader tab in front with no conversation behind it (one restored on
    // launch, say) is still answered by Reply.
    if (!selectedThread && readerTabInFront()) {
      list.push({
        id: 'reply.focus',
        label: 'Reply',
        icon: icon(Reply),
        keywords: 'respond',
        shortcut: 'reply.focus',
        run: run(() => replyInConversation()),
      })
    }

    // Actions on the open conversation — only when one is on screen.
    if (selectedThread && (!activeBoardId || !!kanbanPaneThreadId())) {
      list.push(
        {
          id: 'reply.focus',
          label: 'Reply',
          icon: icon(Reply),
          keywords: 'respond quick',
          shortcut: 'reply.focus',
          run: run(() => replyInConversation()),
        },
        ...(replyAll
          ? [
              {
                id: 'reply.all',
                label: 'Reply all',
                icon: icon(ReplyAll),
                keywords: 'respond everyone cc',
                shortcut: 'reply.all' as const,
                run: run(() => openReplyInFullEditor({ replyAll: true })),
              },
            ]
          : []),
        {
          id: 'compose.replyFull',
          label: 'Reply in full editor',
          icon: icon(Maximize2),
          keywords: 'expand compose respond editor',
          shortcut: 'compose.replyFull',
          run: run(() => openReplyInFullEditor()),
        },
        {
          id: 'thread.archive',
          label: 'Archive thread',
          icon: icon(Archive),
          keywords: 'remove inbox',
          shortcut: 'thread.archive',
          run: run(() => void archiveThread(selectedThread)),
        },
        {
          id: 'thread.star',
          label: 'Toggle star',
          icon: icon(Star),
          keywords: 'flag favorite',
          shortcut: 'thread.star',
          run: run(() => toggleStarWithUndo(selectedThread)),
        },
        {
          id: 'thread.unread',
          label: 'Mark unread',
          icon: icon(MailOpen),
          keywords: 'seen',
          shortcut: 'thread.unread',
          run: run(() => markUnreadWithUndo(selectedThread)),
        },
        {
          id: 'thread.delete',
          label: 'Delete thread',
          icon: icon(Trash2),
          keywords: 'trash remove',
          shortcut: 'thread.delete',
          run: run(() => void deleteThread(selectedThread)),
        },
      )
    }

    // Close the active reader/compose tab when one is open.
    if (activeTab) {
      list.push({
        id: 'tab.close',
        label: 'Close tab',
        icon: icon(X),
        keywords: 'dismiss editor reader',
        shortcut: 'tab.close',
        run: run(() => void closeMessageTab(activeTab)),
      })
    }

    // Switch accounts (plus the unified inbox when more than one exists).
    if (accounts.length > 1) {
      list.push({
        id: 'account.unified',
        label: 'Go to: Unified inbox',
        icon: icon(Users),
        keywords: 'all accounts switch',
        shortcut: railShortcut(1),
        active: !activeBoardId && selectedAccount === 'unified',
        run: run(() => openMailAccount('unified')),
      })
    }
    for (const [accountIndex, account] of accounts.entries()) {
      list.push({
        id: `account.${account.id}`,
        label: `Go to: ${account.display_name || account.email}`,
        icon: icon(Mail),
        keywords: `account switch ${account.email}`,
        shortcut: railShortcut(boards.length + accountIndex + 2),
        active: !activeBoardId && selectedAccount === account.id,
        run: run(() => openMailAccount(account.id)),
      })
    }

    // Jump to a folder in the current account.
    for (const folder of folders) {
      list.push({
        id: `folder.${folder.id}`,
        label: `Folder: ${folder.name}`,
        icon: icon(Inbox),
        keywords: 'open mailbox',
        active: selectedFolder === folder.id,
        run: run(() => {
          leaveNarrowContacts()
          ui$.selectedFolder.set(folder.id)
        }),
      })
    }

    // Individual settings: typed-for only, each opening Settings at its row.
    for (const entry of SETTINGS_INDEX) {
      if (entry.available && !entry.available()) continue
      const title = t(entry.titleKey)
      list.push({
        id: `setting.${entry.titleKey}`,
        label: `${t('settings.label')}: ${title}`,
        icon: icon(SlidersHorizontal),
        keywords: `preferences ${entry.keywords ?? ''}`,
        searchOnly: true,
        run: run(() => openSettingsAt(entry.page, title)),
      })
    }

    // Each account's and board's own settings page, then its rows. Listed after
    // the app-wide ones, so "signature" shows the global one first and then
    // where each account overrides it.
    const settingsLabel = t('settings.label')
    for (const account of accounts) {
      const name = account.display_name || account.email
      const kind = accountKind(account)
      const accountWords = `account ${account.email} ${account.display_name ?? ''}`
      list.push({
        id: `setting.account.${account.id}`,
        label: `${settingsLabel}: ${name}`,
        icon: icon(SlidersHorizontal),
        keywords: `preferences ${accountWords}`,
        searchOnly: true,
        run: run(() => openSettingsAt(account.id, '')),
      })
      for (const entry of ACCOUNT_SETTINGS_INDEX) {
        if (!entry.kinds.includes(kind) || (entry.available && !entry.available(account))) continue
        const title = t(entry.titleKey)
        list.push({
          id: `setting.account.${account.id}.${entry.titleKey}`,
          label: `${settingsLabel}: ${name} › ${title}`,
          icon: icon(SlidersHorizontal),
          keywords: `preferences ${accountWords} ${entry.keywords ?? ''}`,
          searchOnly: true,
          run: run(() => openSettingsAt(account.id, title)),
        })
      }
    }
    for (const board of boards) {
      list.push({
        id: `setting.board.${board.id}`,
        label: `${settingsLabel}: ${board.name}`,
        icon: icon(SlidersHorizontal),
        keywords: 'preferences kanban board',
        searchOnly: true,
        run: run(() => openSettingsAt(board.id, '')),
      })
      for (const entry of BOARD_SETTINGS_INDEX) {
        const title = t(entry.titleKey)
        list.push({
          id: `setting.board.${board.id}.${entry.titleKey}`,
          label: `${settingsLabel}: ${board.name} › ${title}`,
          icon: icon(SlidersHorizontal),
          keywords: `preferences kanban board ${entry.keywords ?? ''}`,
          searchOnly: true,
          run: run(() => openSettingsAt(board.id, title)),
        })
      }
    }

    return list
  }, [
    t,
    boards,
    activeBoardId,
    chosenThemeIds,
    customThemes,
    filterMode,
    kanbanFilterMode,
    attachmentsOnly,
    kanbanAttachmentsOnly,
    selectedAccount,
    selectedFolder,
    selectedThread,
    accounts,
    folders,
    activeTab,
    replyAll,
    calendarEnabled,
    conversationLayout,
    appIconSupported,
    titlebarSupported,
    updateSupported,
  ])
}
