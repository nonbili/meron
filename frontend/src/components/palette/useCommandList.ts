import { useMemo } from 'react'
import {
  Archive,
  Columns3,
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
  RefreshCw,
  Search,
  SearchCheck,
  Settings,
  Star,
  Sun,
  Trash2,
  Users,
  X,
} from 'lucide-react'
import { createElement } from 'react'
import { useValue } from '@legendapp/state/react'
import { ui$, closeCommandPalette, focusGlobalSearch, focusQuickReply } from '../../states/ui'
import { selectTheme, settings$ } from '../../states/settings'
import { accounts$ } from '../../states/accounts'
import { BUILTIN_THEMES } from '../../lib/themes'
import {
  closeKanbanBoard,
  createKanbanBoard,
  kanban$,
  selectKanbanBoard,
  setGlobalKanbanFilter,
} from '../../states/kanban'
import { thread$ } from '../../states/thread'
import {
  mail$,
  syncMail,
  markAllRead,
  archiveThread,
  deleteThread,
  toggleStarWithUndo,
  markUnreadWithUndo,
} from '../../states/mail'
import { compose$, openComposeTab, openReplyInFullEditor, closeMessageTab } from '../../states/compose'
import { RAIL_SHORTCUT_IDS, type ShortcutId } from '../../lib/shortcuts'
import type { Command } from './paletteCommands'

// Builds the full, ordered command list from current app state. Each entry's
// `run` closes the palette first, then performs its action. Returns a memoized
// array recomputed only when the underlying selections change.
export function useCommandList(): Command[] {
  const boards = useValue(settings$.kanbanBoards)
  const activeBoardId = useValue(kanban$.activeBoardId)
  const themeId = useValue(settings$.themeId)
  const customThemes = useValue(settings$.customThemes)
  const filterMode = useValue(ui$.filterMode)
  const kanbanFilterMode = useValue(kanban$.globalFilter)
  const selectedAccount = useValue(ui$.selectedAccount)
  const selectedFolder = useValue(ui$.selectedFolder)
  const selectedThread = useValue(ui$.selectedThread)
  const accounts = useValue(accounts$)
  const folders = useValue(mail$.folders)
  const activeTab = useValue(compose$.activeTab)

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
    const railShortcut = (slot: number) => RAIL_SHORTCUT_IDS[slot - 1] as ShortcutId | undefined

    const list: Command[] = [
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
        run: run(() => void markAllRead()),
      },
      {
        id: 'search.thread',
        label: 'Search current thread',
        icon: icon(Search),
        keywords: 'find conversation in',
        shortcut: 'search.thread',
        run: run(() => {
          const visible = !!selectedThread && (!activeBoardId || !!kanban$.paneThreadId.peek())
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
        run: run(() => closeKanbanBoard()),
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
      ...[...BUILTIN_THEMES, ...customThemes].map((theme) => ({
        id: `theme.${theme.id}`,
        label: `Theme: ${theme.name}`,
        icon: icon(theme.appearance === 'light' ? Sun : Moon),
        active: themeId === theme.id,
        run: run(() => selectTheme(theme)),
      })),
    ]

    // Actions on the open conversation — only when one is on screen.
    if (selectedThread && (!activeBoardId || !!kanban$.paneThreadId.peek())) {
      list.push(
        {
          id: 'reply.focus',
          label: 'Reply',
          icon: icon(Reply),
          keywords: 'respond quick',
          shortcut: 'reply.focus',
          run: run(() => focusQuickReply()),
        },
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
        run: run(() => closeMessageTab(activeTab)),
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
        run: run(() => {
          closeKanbanBoard()
          ui$.selectedAccount.set('unified')
          ui$.selectedFolder.set('inbox')
        }),
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
        run: run(() => {
          closeKanbanBoard()
          ui$.selectedAccount.set(account.id)
          ui$.selectedFolder.set('inbox')
        }),
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
        run: run(() => ui$.selectedFolder.set(folder.id)),
      })
    }

    return list
  }, [
    boards,
    activeBoardId,
    themeId,
    customThemes,
    filterMode,
    kanbanFilterMode,
    selectedAccount,
    selectedFolder,
    selectedThread,
    accounts,
    folders,
    activeTab,
  ])
}
