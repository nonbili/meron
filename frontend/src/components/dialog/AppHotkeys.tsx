import { useEffect } from 'react'
import {
  clearBulkSelection,
  selectedBulkItems,
  ui$,
  openCommandPalette,
  closeCommandPalette,
  focusGlobalSearch,
  focusQuickReply,
} from '../../states/ui'
import { settings$, visibleSideNavAccounts } from '../../states/settings'
import { accounts$ } from '../../states/accounts'
import {
  kanban$,
  selectAdjacentKanbanThread,
  selectKanbanBoard,
  closeKanbanBoard,
  closeKanbanPane,
} from '../../states/kanban'
import { thread$ } from '../../states/thread'
import {
  syncMail,
  selectAdjacentThread,
  archiveThread,
  deleteThread,
  toggleStarWithUndo,
  markUnreadWithUndo,
  bulkArchiveSelected,
  bulkDeleteSelected,
  bulkMarkSelectedUnread,
  bulkStarSelected,
} from '../../states/mail'
import { compose$, openComposeTab, openReplyInFullEditor, closeMessageTab } from '../../states/compose'
import {
  isBareShortcut,
  matchShortcut,
  RAIL_SHORTCUT_IDS,
  type RailShortcutId,
  type ShortcutId,
} from '../../lib/shortcuts'

// Whether the conversation pane (and thus its in-thread search) is on screen.
function threadSearchVisible(): boolean {
  if (!ui$.selectedThread.peek()) return false
  return !kanban$.activeBoardId.peek() || !!kanban$.paneThreadId.peek()
}

// Is the user currently typing into a field? Bare single-key shortcuts must
// stand down so they don't clobber text entry.
function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null
  if (!el) return false
  return el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable
}

function modalOpen(): boolean {
  return (
    ui$.paletteOpen.peek() ||
    ui$.shortcutsOpen.peek() ||
    ui$.settingsOpen.peek() ||
    ui$.setupOpen.peek() ||
    !!ui$.addFeedAccount.peek() ||
    !!ui$.editFeed.peek() ||
    !!ui$.confirm.peek()
  )
}

// Bare shortcuts are thread-list/conversation actions, so they only apply in the
// chat layout with nothing modal in the way.
function bareShortcutsActive(): boolean {
  if (kanban$.activeBoardId.peek()) return false
  return !modalOpen()
}

function kanbanArrowNavigationActive(): boolean {
  return !!kanban$.activeBoardId.peek() && !modalOpen()
}

function handleKanbanArrowNavigation(key: string, target: EventTarget | null): boolean {
  if ((key !== 'ArrowDown' && key !== 'ArrowUp') || isTyping(target) || !kanbanArrowNavigationActive()) {
    return false
  }
  selectAdjacentKanbanThread(key === 'ArrowDown' ? 1 : -1)
  return true
}

let threadListDeleteScope = false

function inThreadList(el: Element | null): boolean {
  return !!el?.closest('[data-thread-list]')
}

function bodyFocused(): boolean {
  const el = document.activeElement
  return !el || el === document.body || el === document.documentElement
}

function clearThreadListDeleteScope(event: Event) {
  if (!inThreadList(event.target as Element | null)) threadListDeleteScope = false
}

// Delete is destructive, so unlike the other bare shortcuts it requires focus
// to be inside the thread list. After deleting the focused row, browsers can
// drop focus to body; keep allowing repeated Delete only until the user focuses
// or clicks outside the thread list.
function deleteKeyFocusAllowed(): boolean {
  const el = document.activeElement
  if (inThreadList(el)) {
    threadListDeleteScope = true
    return true
  }
  return bodyFocused() && threadListDeleteScope
}

// Delete removes the selected thread, same as the context menu action —
// deleteThread itself picks move-to-trash vs permanent based on the folder.
// Active in both the chat and kanban views (selection lives in
// ui$.selectedThread either way), unlike the other bare shortcuts.
function handleDeleteKey(key: string, target: EventTarget | null): boolean {
  if (key !== 'Delete' || isTyping(target) || modalOpen() || !deleteKeyFocusAllowed()) return false
  const threadId = ui$.selectedThread.peek()
  const bulkItems = selectedBulkItems()
  if (bulkItems.length > 0) {
    void bulkDeleteSelected(bulkItems)
    return true
  }
  if (!threadId) return false
  void deleteThread(threadId)
  return true
}

// Arrow up/down steps through the chat thread list, just like j/k.
function handleChatArrowNavigation(key: string, target: EventTarget | null): boolean {
  if ((key !== 'ArrowDown' && key !== 'ArrowUp') || isTyping(target) || !bareShortcutsActive()) {
    return false
  }
  selectAdjacentThread(key === 'ArrowDown' ? 1 : -1)
  return true
}

function handleRailShortcut(action: ShortcutId): boolean {
  const slot = RAIL_SHORTCUT_IDS.indexOf(action as RailShortcutId)
  if (slot === -1 || modalOpen()) return false

  let target = slot
  if (settings$.showUnifiedInboxInSideNav.peek()) {
    if (target === 0) {
      closeKanbanBoard()
      ui$.selectedAccount.set('unified')
      ui$.selectedFolder.set('inbox')
      return true
    }
    target -= 1
  }

  const boards = settings$.kanbanBoards.peek()
  const board = boards[target]
  if (board) {
    selectKanbanBoard(board.id)
    return true
  }
  target -= boards.length

  const account = visibleSideNavAccounts(accounts$.peek())[target]
  if (!account) return false
  closeKanbanBoard()
  ui$.selectedAccount.set(account.id)
  ui$.selectedFolder.set('inbox')
  return true
}

/**
 * Mounts the global keyboard shortcuts. Renders nothing. ⌘/Ctrl-modified
 * shortcuts work everywhere (even while typing); bare single-key shortcuts
 * (j/k/e/s/u/#/r) only fire in the chat view when not typing and no modal is
 * open. Per-palette navigation (arrows/enter/esc) lives inside CommandPalette.
 */
export function AppHotkeys() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isTyping(event.target) && !modalOpen() && selectedBulkItems().length > 0) {
        event.preventDefault()
        clearBulkSelection()
        return
      }

      // Esc closes the open kanban conversation pane (it has no thread list to
      // fall back to). Skip while typing or when a modal owns the keystroke;
      // in-thread search handles its own Esc via the input's onKeyDown.
      if (
        event.key === 'Escape' &&
        !isTyping(event.target) &&
        !modalOpen() &&
        kanban$.activeBoardId.peek() &&
        kanban$.paneThreadId.peek()
      ) {
        event.preventDefault()
        closeKanbanPane()
        return
      }

      if (
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey &&
        !event.shiftKey &&
        (handleKanbanArrowNavigation(event.key, event.target) ||
          handleChatArrowNavigation(event.key, event.target) ||
          handleDeleteKey(event.key, event.target))
      ) {
        event.preventDefault()
        return
      }

      const action = matchShortcut(event)
      if (!action) return

      if (handleRailShortcut(action)) {
        event.preventDefault()
        return
      }

      // While the command palette is open, its own key handler owns navigation
      // and execution. Do not let global chords such as Ctrl+N open composer.
      if (ui$.paletteOpen.peek() && action !== 'palette.open') return

      // Gate single-key shortcuts: never steal a keystroke from a text field,
      // and only act in the chat view with no modal open.
      if (isBareShortcut(action)) {
        if (isTyping(event.target) || !bareShortcutsActive()) return
      }

      const selected = () => ui$.selectedThread.peek()
      const bulkSelected = () => selectedBulkItems()

      switch (action) {
        case 'palette.open':
          event.preventDefault()
          if (ui$.paletteOpen.peek()) closeCommandPalette()
          else openCommandPalette()
          break
        case 'compose.new':
          event.preventDefault()
          openComposeTab()
          break
        case 'settings.open':
          event.preventDefault()
          ui$.settingsOpen.set(true)
          break
        case 'mail.sync':
          event.preventDefault()
          void syncMail()
          break
        case 'view.toggle': {
          event.preventDefault()
          const activeBoard = kanban$.activeBoardId.peek()
          if (activeBoard) {
            closeKanbanBoard()
          } else {
            const board = settings$.kanbanBoards.peek()[0]
            if (board) selectKanbanBoard(board.id)
          }
          break
        }
        case 'search.thread':
          // Find in the open conversation; fall back to the thread-list search
          // when no conversation is on screen. preventDefault stops the webview's
          // native ⌘/Ctrl+F find bar.
          event.preventDefault()
          if (threadSearchVisible()) thread$.searchOpen.set(true)
          else focusGlobalSearch()
          break
        case 'search.global':
          event.preventDefault()
          focusGlobalSearch()
          break
        case 'compose.replyFull':
          // Only meaningful with a conversation on screen; openReplyInFullEditor
          // no-ops otherwise, but gating avoids stealing ⌘/Ctrl+E elsewhere.
          if (!threadSearchVisible()) return
          event.preventDefault()
          openReplyInFullEditor()
          break
        case 'tab.close': {
          // Close the active reader/compose tab. When none is open, let ⌘/Ctrl+W
          // pass through to its default (e.g. closing the window).
          const tab = compose$.activeTab.peek()
          if (!tab) return
          event.preventDefault()
          closeMessageTab(tab)
          break
        }
        case 'shortcuts.help':
          event.preventDefault()
          ui$.shortcutsOpen.set(!ui$.shortcutsOpen.peek())
          break
        case 'thread.next':
          event.preventDefault()
          selectAdjacentThread(1)
          break
        case 'thread.prev':
          event.preventDefault()
          selectAdjacentThread(-1)
          break
        case 'thread.archive':
          if (bulkSelected().length > 0) {
            event.preventDefault()
            void bulkArchiveSelected(bulkSelected())
            break
          }
          if (!selected()) return
          event.preventDefault()
          void archiveThread(selected())
          break
        case 'thread.star':
          if (bulkSelected().length > 0) {
            event.preventDefault()
            void bulkStarSelected(bulkSelected(), true)
            break
          }
          if (!selected()) return
          event.preventDefault()
          toggleStarWithUndo(selected())
          break
        case 'thread.unread':
          if (bulkSelected().length > 0) {
            event.preventDefault()
            void bulkMarkSelectedUnread(bulkSelected())
            break
          }
          if (!selected()) return
          event.preventDefault()
          markUnreadWithUndo(selected())
          break
        case 'thread.delete':
          if (bulkSelected().length > 0) {
            event.preventDefault()
            void bulkDeleteSelected(bulkSelected())
            break
          }
          if (!selected()) return
          event.preventDefault()
          void deleteThread(selected())
          break
        case 'thread.details':
          if (!threadSearchVisible()) return
          event.preventDefault()
          thread$.mediaOpen.set(!thread$.mediaOpen.peek())
          break
        case 'reply.focus':
          if (!threadSearchVisible()) return
          event.preventDefault()
          focusQuickReply()
          break
      }
    }

    const onFrameKeyDown = (event: Event) => {
      const detail = (event as CustomEvent<{ key?: string }>).detail
      if (detail?.key && handleKanbanArrowNavigation(detail.key, null)) {
        event.preventDefault()
      }
    }

    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('focusin', clearThreadListDeleteScope)
    window.addEventListener('pointerdown', clearThreadListDeleteScope)
    window.addEventListener('meron.frameKeyDown', onFrameKeyDown)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('focusin', clearThreadListDeleteScope)
      window.removeEventListener('pointerdown', clearThreadListDeleteScope)
      window.removeEventListener('meron.frameKeyDown', onFrameKeyDown)
    }
  }, [])

  return null
}
