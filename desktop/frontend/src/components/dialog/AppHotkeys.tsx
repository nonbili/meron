import { useEffect } from 'react'
import { leaveNarrowContacts } from '../../states/contacts'
import {
  clearBulkSelection,
  selectedBulkItems,
  ui$,
  openCommandPalette,
  closeCommandPalette,
  focusGlobalSearch,
  focusQuickReply,
  modalOpen,
} from '../../states/ui'
import { settings$, visibleSideNavAccounts } from '../../states/settings'
import { accounts$ } from '../../states/accounts'
import {
  kanban$,
  selectAdjacentKanbanThread,
  selectKanbanBoard,
  closeKanbanBoard,
  openMailAccount,
  kanbanPaneThreadId,
  kanbanKeyboardThreadId,
  type KanbanCardRef,
} from '../../states/kanban'
import { closeConversationToCard } from '../kanban/closeConversationToCard'
import { openThreadSearch, thread$ } from '../../states/thread'
import { getActiveThread, syncMail, selectAdjacentThread } from '../../states/mail'
import {
  toggleStarWithUndo,
  markUnreadWithUndo,
  bulkMarkSelectedUnread,
  bulkStarSelected,
} from '../../states/mailFlags'
import { archiveThread, deleteThread, bulkArchiveSelected, bulkDeleteSelected } from '../../states/mailMoves'
import { openComposeTab, openReplyInFullEditor, closeMessageTab, toggleConversationMode } from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { closeComposeSessionKeepingDraft } from '../../states/composeSessions'
import { calendar$ } from '../../states/calendar'
import { handleEditUndoKeyDown } from '../../lib/editUndo'
import { closeTopEscapeLayer } from '../../lib/useEscapeKey'
import { inMessageScroller, pageOpenMessage, stepOpenMessage } from '../chat/messagePaging'
import {
  isArrowKey,
  isBareKeystroke,
  isMac,
  isTyping,
  matchShortcut,
  spaceActivatesTarget,
  shortcutForChord,
  RAIL_SHORTCUT_IDS,
  type Chord,
  type RailShortcutId,
  type ShortcutId,
} from '../../lib/shortcuts'

// Whether the conversation pane (and thus its in-thread search) is on screen.
function threadSearchVisible(): boolean {
  if (!ui$.selectedThread.peek()) return false
  return !kanban$.activeBoardId.peek() || !!kanbanPaneThreadId()
}

// The full calendar replaces mail; the event editor (from either calendar,
// the panel included) is a modal over it.
function mailCovered(): boolean {
  return ui$.calendarOpen.peek() || !!calendar$.editor.peek()
}

// Bare shortcuts are thread-list/conversation actions. They apply in the kanban
// view too — the selected card lives in ui$.selectedThread, same as a chat-list
// row — as long as nothing modal is in the way.
function bareShortcutsActive(): boolean {
  return !modalOpen() && !calendar$.editor.peek()
}

function kanbanArrowNavigationActive(): boolean {
  return !!kanban$.activeBoardId.peek() && !modalOpen() && !mailCovered()
}

function chatArrowNavigationActive(): boolean {
  return !kanban$.activeBoardId.peek() && !modalOpen() && !mailCovered()
}

// Shortcuts that act on the mail on screen. The calendar takes the place of
// mail (and of a board) while keeping their selection for when it closes, so
// these must not reach mail the user can no longer see.
const MAIL_VIEW_ACTIONS = new Set<ShortcutId>([
  'thread.next',
  'thread.prev',
  'message.pageDown',
  'message.pageUp',
  'thread.archive',
  'thread.star',
  'thread.unread',
  'thread.delete',
  'thread.details',
  'thread.viewMode',
  'reply.focus',
  'reply.all',
  'compose.replyFull',
  'search.thread',
])

// The thread a bare action applies to. On a board it's the one the pane shows:
// an RSS card opens in a reader tab without retargeting ui$.selectedThread,
// which would otherwise leave a stale chat-list selection as the target. With
// nothing in the pane, it's the card selected from the keyboard.
function activeThreadId(): string {
  if (kanban$.activeBoardId.peek()) return kanbanPaneThreadId() || kanbanKeyboardThreadId() || ui$.selectedThread.peek()
  return ui$.selectedThread.peek()
}

// j/k (and the arrow keys) step through whichever list is on screen: the chat
// thread list, or the active board's cards.
function selectAdjacentThreadForView(delta: number) {
  if (kanban$.activeBoardId.peek()) selectAdjacentKanbanThread(delta, focusedKanbanCard())
  else selectAdjacentThread(delta)
}

// The card keyboard focus is on (Esc puts it on the card whose conversation it
// closed), for stepping on from there while no conversation is open.
function focusedKanbanCard(): KanbanCardRef | undefined {
  const card = document.activeElement?.closest<HTMLElement>('[data-kanban-thread]')
  const column = card?.closest<HTMLElement>('[data-kanban-column]')
  const threadId = card?.dataset.kanbanThread
  const columnKey = column?.dataset.kanbanColumn
  const itemId = card?.dataset.kanbanItem
  if (!threadId || !columnKey) return undefined
  return itemId ? { threadId, columnKey, itemId } : { threadId, columnKey }
}

function handleKanbanArrowNavigation(key: string, target: EventTarget | null): boolean {
  if (
    (key !== 'ArrowDown' && key !== 'ArrowUp') ||
    isTyping(target) ||
    focusInMessage(target) ||
    !kanbanArrowNavigationActive()
  ) {
    return false
  }
  selectAdjacentKanbanThread(key === 'ArrowDown' ? 1 : -1, focusedKanbanCard())
  return true
}

// Esc in the message hands the keyboard back to the thread list, where the
// plain arrow keys step threads again. Nothing to do when the list isn't on
// screen (a narrow window showing only the conversation).
function focusActiveThreadRow(): boolean {
  const row = document.querySelector<HTMLElement>('[data-thread-list] [data-thread-row][data-active]')
  if (!row || row.getClientRects().length === 0) return false
  row.focus()
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

// Whether the reader last clicked or focused into the message, where the plain
// arrow keys scroll it rather than step the thread list. Tracked rather than
// read off document.activeElement alone: focus falls to <body> whenever the
// focused element unmounts — expanding or collapsing a message swaps its card
// — and the arrows must not snap back to the list then.
let keyboardInMessage = false

function trackKeyboardRegion(event: Event) {
  keyboardInMessage = inMessageScroller(event.target)
}

function focusInMessage(target: EventTarget | null): boolean {
  return inMessageScroller(target) || (keyboardInMessage && bodyFocused())
}

// Arrow up/down in the message scroll it. While focus is inside, the browser
// does that on its own; once focus has fallen to <body>, step it here.
function handleMessageArrowStep(key: string): boolean {
  if ((key !== 'ArrowDown' && key !== 'ArrowUp') || !keyboardInMessage || !bodyFocused()) return false
  if (modalOpen() || mailCovered()) return false
  if (stepOpenMessage(key === 'ArrowDown' ? 1 : -1)) return true
  // The message has gone (the conversation closed): the list gets them back.
  keyboardInMessage = false
  return false
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
  if (key !== 'Delete' || isTyping(target) || modalOpen() || mailCovered() || !deleteKeyFocusAllowed()) return false
  const threadId = activeThreadId()
  const bulkItems = selectedBulkItems()
  if (bulkItems.length > 0) {
    void bulkDeleteSelected(bulkItems)
    return true
  }
  if (!threadId) return false
  void deleteThread(threadId)
  return true
}

// Arrow up/down steps through the chat thread list, just like j/k — unless
// the reader is in the message, where they scroll it (handleMessageArrowStep).
function handleChatArrowNavigation(key: string, target: EventTarget | null): boolean {
  if (
    (key !== 'ArrowDown' && key !== 'ArrowUp') ||
    isTyping(target) ||
    focusInMessage(target) ||
    !chatArrowNavigationActive()
  ) {
    return false
  }
  selectAdjacentThread(key === 'ArrowDown' ? 1 : -1)
  return true
}

function handleRailShortcut(action: ShortcutId): boolean {
  const slot = RAIL_SHORTCUT_IDS.indexOf(action as RailShortcutId)
  if (slot === -1 || modalOpen() || calendar$.editor.peek()) return false

  let target = slot
  if (settings$.showUnifiedInboxInSideNav.peek()) {
    if (target === 0) {
      openMailAccount('unified')
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
  openMailAccount(account.id)
  return true
}

/**
 * Mounts the global keyboard shortcuts. Renders nothing. ⌘/Ctrl-modified
 * shortcuts work everywhere (even while typing); bare single-key shortcuts
 * (j/k/e/s/u/#/i/r, Space) fire in the chat and kanban views when not typing and no
 * modal is open. Per-palette navigation (arrows/enter/esc) lives inside
 * CommandPalette.
 */
export function AppHotkeys() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isTyping(event.target) && !modalOpen() && selectedBulkItems().length > 0) {
        event.preventDefault()
        clearBulkSelection()
        return
      }

      // Esc with a compose tab in front closes it and keeps the message in
      // Drafts, before anything behind it closes. The composer handles the key
      // itself while focus is inside it; this covers focus left elsewhere,
      // e.g. on a footer button WebKit doesn't focus when clicked.
      if (
        event.key === 'Escape' &&
        !isTyping(event.target) &&
        !modalOpen() &&
        !mailCovered() &&
        closeComposeSessionKeepingDraft(compose$.activeTab.peek())
      ) {
        event.preventDefault()
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
        closeConversationToCard()
        return
      }

      if (
        event.key === 'Escape' &&
        !event.defaultPrevented &&
        focusInMessage(event.target) &&
        !isTyping(event.target) &&
        !modalOpen() &&
        !mailCovered() &&
        focusActiveThreadRow()
      ) {
        event.preventDefault()
        return
      }

      const action = matchShortcut(event)

      // Undo/redo inside plain text fields, which the Linux webview leaves
      // unbound. Gated on `!action` so a shortcut rebound onto the chord still
      // wins, same precedence as the arrow keys below.
      if (!action && handleEditUndoKeyDown(event)) return

      // Built-in list navigation on the arrow and Delete keys. No default chord
      // uses them, but a user can rebind a shortcut onto one — that binding wins,
      // so the row below never runs behind its back.
      if (
        !action &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey &&
        !event.shiftKey &&
        (handleMessageArrowStep(event.key) ||
          handleKanbanArrowNavigation(event.key, event.target) ||
          handleChatArrowNavigation(event.key, event.target) ||
          handleDeleteKey(event.key, event.target))
      ) {
        event.preventDefault()
        return
      }

      if (!action) return

      if (handleRailShortcut(action)) {
        event.preventDefault()
        return
      }

      // While the command palette is open, its own key handler owns navigation
      // and execution. Do not let global chords such as Ctrl+N open composer.
      if (ui$.paletteOpen.peek() && action !== 'palette.open') return

      // Gate single-key shortcuts: never steal a keystroke from a text field,
      // and only act in the chat view with no modal open. Arrow chords too:
      // Alt/Option+arrow moves the caret by word or paragraph.
      if (isBareKeystroke(event) || isArrowKey(event.key)) {
        if (isTyping(event.target) || !bareShortcutsActive()) return
      }

      if (MAIL_VIEW_ACTIONS.has(action) && mailCovered()) return

      const selected = () => activeThreadId()
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
            leaveNarrowContacts()
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
          if (threadSearchVisible()) openThreadSearch()
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
          void closeMessageTab(tab)
          break
        }
        case 'shortcuts.help':
          event.preventDefault()
          ui$.shortcutsOpen.set(!ui$.shortcutsOpen.peek())
          break
        case 'thread.next':
          event.preventDefault()
          selectAdjacentThreadForView(1)
          break
        case 'thread.prev':
          event.preventDefault()
          selectAdjacentThreadForView(-1)
          break
        case 'message.pageDown':
        case 'message.pageUp':
          if (spaceActivatesTarget(event.key, event.target)) return
          if (!pageOpenMessage(action === 'message.pageDown' ? 1 : -1)) return
          event.preventDefault()
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
        case 'thread.viewMode':
          // A reader tab has its own HTML / plain switch, so it counts even
          // when no conversation is open behind it.
          if (!compose$.activeTab.peek() && !threadSearchVisible()) return
          if (!toggleConversationMode(getActiveThread()?.account_id)) return
          event.preventDefault()
          break
        case 'reply.focus':
          if (!threadSearchVisible()) return
          event.preventDefault()
          focusQuickReply()
          break
        case 'reply.all':
          if (!threadSearchVisible()) return
          event.preventDefault()
          openReplyInFullEditor({ replyAll: true })
          break
      }
    }

    // Keys forwarded out of an HTML message iframe, which the global keydown
    // listener never sees: ⌘/Ctrl (or Alt) chords such as ⌘/Ctrl+F, bare
    // single-key shortcuts, plus bare ArrowUp/ArrowDown. The detail is the
    // chord HtmlFrame reconstructed.
    const onFrameKeyDown = (event: Event) => {
      const chord = (event as CustomEvent<Chord | undefined>).detail
      if (!chord?.key) return
      // Keys only come from a frame with focus in it, which is in the message,
      // whether or not the parent saw that focus arrive.
      keyboardInMessage = true

      // Esc from inside the message: back to the list, as from the pane around
      // the frame. On a board that closes the conversation pane.
      if (chord.key === 'Escape') {
        // A preview or menu open over the conversation closes first: focus in
        // the frame kept the key from its own Escape listener.
        if (closeTopEscapeLayer()) {
          event.preventDefault()
          return
        }
        if (modalOpen() || mailCovered()) return
        if (kanban$.activeBoardId.peek() && kanban$.paneThreadId.peek()) closeConversationToCard()
        else if (!focusActiveThreadRow()) return
        event.preventDefault()
        return
      }

      // A shortcut rebound onto the arrow key wins over list navigation, same
      // precedence as in the main handler. Replaying it as a real keydown runs
      // it through the one switch above instead of duplicating the dispatch.
      if (shortcutForChord(chord)) {
        // Cancelable so the switch above can report back whether it acted: a
        // bound chord it declines (⌘/Ctrl+W with no tab open) must keep its
        // native behaviour inside the frame rather than being swallowed here.
        const replay = new KeyboardEvent('keydown', {
          key: chord.key,
          cancelable: true,
          ctrlKey: !!chord.mod && !isMac,
          metaKey: !!chord.mod && isMac,
          shiftKey: !!chord.shift,
          altKey: !!chord.alt,
        })
        if (!window.dispatchEvent(replay)) event.preventDefault()
        return
      }

      // Focus is in the message, so the arrows scroll it, as they do in the
      // pane around the frame. The frame is sized to its content and can't
      // scroll itself.
      if (
        !chord.mod &&
        !chord.alt &&
        !chord.shift &&
        (chord.key === 'ArrowDown' || chord.key === 'ArrowUp') &&
        !modalOpen() &&
        !mailCovered() &&
        stepOpenMessage(chord.key === 'ArrowDown' ? 1 : -1)
      ) {
        event.preventDefault()
      }
    }

    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('focusin', clearThreadListDeleteScope)
    window.addEventListener('pointerdown', clearThreadListDeleteScope)
    window.addEventListener('focusin', trackKeyboardRegion)
    window.addEventListener('pointerdown', trackKeyboardRegion)
    window.addEventListener('meron.frameKeyDown', onFrameKeyDown)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('focusin', clearThreadListDeleteScope)
      window.removeEventListener('pointerdown', clearThreadListDeleteScope)
      window.removeEventListener('focusin', trackKeyboardRegion)
      window.removeEventListener('pointerdown', trackKeyboardRegion)
      window.removeEventListener('meron.frameKeyDown', onFrameKeyDown)
    }
  }, [])

  return null
}
