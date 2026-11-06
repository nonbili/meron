import { useConversationView } from './conversationView'
import { useEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import {
  Archive,
  ChevronDown,
  ChevronLeft,
  ChevronUp,
  ChevronsDownUp,
  ChevronsUpDown,
  Code,
  Download,
  FileText,
  LoaderCircle,
  MoreHorizontal,
  PanelRight,
  Printer,
  Reply,
  ReplyAll,
  Search,
  Star,
  Trash2,
  X,
} from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { printThread } from '../../lib/printMail'
import { useTranslation } from '../../lib/i18n'
import { copyText } from '../../lib/native'
import { showToast, ui$ } from '../../states/ui'
import { settings$ } from '../../states/settings'
import { starThread } from '../../states/mailFlags'
import { deletesForever, isDraftFolder } from '../../states/mailFolders'
import { archiveThread, deleteThread } from '../../states/mailMoves'
import { goToSearchMatch, setAllMessagesExpanded } from '../../states/thread'
import { accounts$, isLocalAccountId } from '../../states/accounts'
import { exportThreadsAsEml } from '../../states/emlExport'
import { isRssAccount } from '../../lib/threadActions'
import { closeCurrentConversation, kanban$ } from '../../states/kanban'
import { openReplyInFullEditor } from '../../states/compose'
import { canReplyAllToThread } from '../../states/composeReply'
import { compose$ } from '../../states/composeState'
import type { Message } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { IconButton } from '../button/IconButton'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import { targetWithin } from '../menu/useDismissOnOutside'
import { AddToTasksButton } from '../tasks/AddToTasksButton'
import { ThreadMoveCopyItems } from '../threads/ThreadMoveCopyItems'
import { ThreadTagItem } from '../threads/ThreadTagItem'
import { ConversationSubject } from './ConversationSubject'
import { useConversationMode } from './useMessageView'

// The conversation header: back/close affordances, the subject, add-to-tasks, the desktop in-thread search box and
// the overflow actions menu (view mode, star, archive, delete). Search match
// state is published to thread$ by useThreadSearch.
export function ConversationHeader({
  activeThread,
  desktopSearchInputRef,
}: {
  activeThread: Message
  desktopSearchInputRef: RefObject<HTMLInputElement | null>
}) {
  const { state: thread$, active: viewActive } = useConversationView()
  const { t } = useTranslation()
  const account = useValue(accounts$).find((acc) => acc.id === activeThread.account_id)
  const isRSS = isRssAccount(account, activeThread.account_id)
  const conversationMode = useConversationMode(activeThread.account_id)
  const matchCount = useValue(thread$.searchMatchCount)
  const activeSearchIndex = useValue(thread$.activeSearchIndex)
  const inKanban = !!useValue(kanban$.activeBoardId)
  // A tab's conversation is closed from the tab strip, not from here: this
  // button closes the conversation the board's card opened.
  const tabOwnsPane = !!useValue(compose$.activeTab)
  const hasTabs = useValue(compose$.tabs).length > 0
  const showCloseConversation = inKanban && !tabOwnsPane && !hasTabs
  const threadSearch = useValue(thread$.search)
  const threadSearchOpen = useValue(thread$.searchOpen)
  const mediaOpen = useValue(thread$.mediaOpen)
  const conversationMenu = useValue(thread$.conversationMenu)
  const expandAllAction = useValue(thread$.expandAllAction)
  const tasksEnabled = useValue(settings$.tasksEnabled)
  const normalizedThreadSearch = threadSearch.trim().toLowerCase()

  const [printing, setPrinting] = useState(false)
  const [actionsMenuOpen, setActionsMenuOpen] = useState(false)
  useEffect(() => {
    if (!viewActive) setActionsMenuOpen(false)
  }, [viewActive])
  const actionsMenuRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    if (!actionsMenuOpen) return
    const onDown = (event: MouseEvent) => {
      // The move and copy flyouts are portalled out of the menu but belong to it.
      if (targetWithin(event.target, '[data-thread-context-menu]')) return
      if (actionsMenuRef.current && !actionsMenuRef.current.contains(event.target as Node)) {
        setActionsMenuOpen(false)
      }
    }
    window.addEventListener('mousedown', onDown)
    return () => window.removeEventListener('mousedown', onDown)
  }, [actionsMenuOpen])

  const copyHeaderText = (text: string, toast: string) => {
    const value = text.trim()
    if (!value) return
    copyText(value)
      .then(() => {
        showToast(toast)
      })
      .catch(() => undefined)
  }

  // Hidden when the conversation's reply target has no other recipients: a
  // reply-all identical to the reply is a second name for the same action.
  const replyAll = useValue(canReplyAllToThread)

  // The same wording, and the same permanence in Trash, as the thread list's menu.
  const inTrash = deletesForever(activeThread.account_id, activeThread.folder_id)
  const inDrafts = isDraftFolder(activeThread.folder_id, activeThread.account_id)
  const closeActionMenus = () => {
    setActionsMenuOpen(false)
    thread$.conversationMenu.set(null)
  }
  // The thread's actions, shown both under the header's button and, for a
  // right-click on the blank part of the conversation, at the pointer.
  const actionItems = (
    <>
      <button
        onClick={() => {
          if (account) thread$.conversationModeOverrides[account.id].set('html')
          closeActionMenus()
        }}
        className={`flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs cursor-pointer hover:bg-hover ${
          conversationMode === 'html' ? 'font-semibold text-accent' : 'font-medium text-primary'
        }`}
      >
        <Code size={15} className="shrink-0" /> {t('chat.viewAsHtml')}
      </button>
      <button
        onClick={() => {
          if (account) thread$.conversationModeOverrides[account.id].set('plain')
          closeActionMenus()
        }}
        className={`flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs cursor-pointer hover:bg-hover ${
          conversationMode === 'plain' ? 'font-semibold text-accent' : 'font-medium text-primary'
        }`}
      >
        <FileText size={15} className="shrink-0" /> {t('chat.viewAsPlainText')}
      </button>
      {expandAllAction && (
        <button
          onClick={() => {
            setAllMessagesExpanded(expandAllAction === 'expand')
            closeActionMenus()
          }}
          className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-primary cursor-pointer hover:bg-hover"
        >
          {expandAllAction === 'expand' ? (
            <ChevronsUpDown size={15} className="shrink-0" />
          ) : (
            <ChevronsDownUp size={15} className="shrink-0" />
          )}
          {expandAllAction === 'expand' ? t('chat.expandAll') : t('chat.collapseAll')}
        </button>
      )}
      <div className="my-1 h-px bg-border" />
      {/* Local conversations have no quick reply box: reply from the full editor. */}
      {!isRSS && isLocalAccountId(activeThread.account_id) && (
        <button
          onClick={() => {
            openReplyInFullEditor()
            closeActionMenus()
          }}
          className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-primary cursor-pointer hover:bg-hover"
        >
          <Reply size={15} className="shrink-0" /> {t('chat.actions.reply')}
        </button>
      )}
      {!isRSS && replyAll && (
        <button
          onClick={() => {
            openReplyInFullEditor({ replyAll: true })
            closeActionMenus()
          }}
          className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-primary cursor-pointer hover:bg-hover"
        >
          <ReplyAll size={15} className="shrink-0" /> {t('chat.actions.replyAll')}
        </button>
      )}
      <button
        onClick={() => {
          void starThread(activeThread.thread_id, !activeThread.starred)
          closeActionMenus()
        }}
        className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-primary cursor-pointer hover:bg-hover"
      >
        <Star size={15} className={`shrink-0 ${activeThread.starred ? 'fill-amber-500 text-amber-500' : ''}`} />
        {activeThread.starred ? t('chat.unstar') : t('chat.star')}
      </button>
      {!isRSS && (
        <>
          <ThreadTagItem
            key={activeThread.thread_id}
            threadId={activeThread.thread_id}
            close={closeActionMenus}
            itemClassName="h-auto! gap-2.5! px-3! py-2 text-xs! font-medium!"
            iconSize={15}
          />
          {!isLocalAccountId(activeThread.account_id) && (
            <button
              onClick={() => {
                void archiveThread(activeThread.thread_id)
                closeActionMenus()
              }}
              className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-primary cursor-pointer hover:bg-hover"
            >
              <Archive size={15} className="shrink-0" /> {t('threads.actions.archiveThread')}
            </button>
          )}
          <ThreadMoveCopyItems
            threadId={activeThread.thread_id}
            accountId={activeThread.account_id}
            folderId={activeThread.folder_id}
            close={closeActionMenus}
            itemClassName="h-auto! gap-2.5! px-3! py-2 text-xs! font-medium!"
            iconSize={15}
          />
          <div className="my-1 h-px bg-border" />
          <MenuItem
            icon={<Printer size={15} className="shrink-0" />}
            label={t('chat.actions.printThread')}
            disabled={printing}
            className="h-auto! gap-2.5! px-3! py-2 text-xs! font-medium! disabled:opacity-50 disabled:cursor-default disabled:hover:bg-transparent"
            onClick={() => {
              closeActionMenus()
              setPrinting(true)
              void printThread(activeThread.thread_id).finally(() => setPrinting(false))
            }}
          />
          <MenuItem
            icon={<Download size={15} className="shrink-0" />}
            label={t('chat.actions.saveAsEml')}
            className="h-auto! gap-2.5! px-3! py-2 text-xs! font-medium!"
            onClick={() => {
              closeActionMenus()
              void exportThreadsAsEml(
                [{ threadId: activeThread.thread_id, folderId: activeThread.folder_id }],
                activeThread.subject || '',
              )
            }}
          />
          <div className="my-1 h-px bg-border" />
          <button
            onClick={() => {
              void deleteThread(activeThread.thread_id, { permanent: inTrash })
              closeActionMenus()
            }}
            className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-rose-600 dark:text-rose-400 cursor-pointer hover:bg-rose-50 dark:hover:bg-rose-950/25"
          >
            <Trash2 size={15} className="shrink-0" />
            {inTrash
              ? t('threads.actions.deleteForever')
              : inDrafts
                ? t('threads.actions.discardDraft')
                : t('threads.actions.moveToTrash')}
          </button>
        </>
      )}
    </>
  )

  return (
    <>
      <header
        className={`relative z-40 flex h-12 shrink-0 items-center gap-3 border-b border-border/60 bg-header select-none ${showCloseConversation ? 'px-2' : 'px-4'}`}
      >
        {printing && (
          <LoaderCircle
            size={16}
            className="shrink-0 animate-spin text-secondary"
            role="status"
            aria-label={t('chat.actions.printThread')}
          />
        )}
        <button
          className="flex h-8 w-8 items-center justify-center rounded-full hover:bg-hover text-secondary min-[769px]:hidden cursor-pointer"
          onClick={() => ui$.mobilePane.set('threads')}
          title={t('chat.backToChats')}
        >
          <ChevronLeft size={20} />
        </button>

        {showCloseConversation && (
          <button
            className="-mr-2 flex h-8 w-8 shrink-0 items-center justify-center rounded-full hover:bg-hover text-secondary cursor-pointer max-[768px]:hidden"
            onClick={closeCurrentConversation}
            title={t('chat.closeConversationEsc')}
          >
            <X size={18} />
          </button>
        )}

        <Avatar
          name={activeThread.from_name || activeThread.from_addr}
          email={isRSS ? undefined : activeThread.from_addr}
          src={isRSS && activeThread.feed_icon ? `/media/${activeThread.feed_icon}` : undefined}
          size={32}
        />

        <div className="min-w-0 flex-1">
          <ConversationSubject
            subject={activeThread.subject}
            copyLabel={t('chat.copySubject')}
            onCopy={() => copyHeaderText(activeThread.subject, 'Subject copied')}
          />
          {isRSS && (
            // RSS subject == from_name (both the feed title), so the feed host
            // sits under it rather than the name again.
            <p className="truncate text-[0.71875rem] text-secondary mt-0.5 font-medium" title={activeThread.from_addr}>
              {activeThread.from_addr}
            </p>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {tasksEnabled && !isRSS && (
            <AddToTasksButton
              title={activeThread.subject}
              account={activeThread.account_id}
              threadId={activeThread.thread_id}
            />
          )}
          {!threadSearchOpen ? (
            <IconButton icon={Search} label={t('chat.searchThread')} onClick={() => thread$.searchOpen.set(true)} />
          ) : (
            <div
              className="hidden min-[900px]:flex w-[286px] items-center gap-1 rounded-xl bg-hover px-2 py-1.5 border border-transparent focus-within:border-accent/40 focus-within:bg-chats"
              // Folds back to the icon once focus leaves it empty, like the kanban
              // board's search; moving between its own buttons keeps it open.
              onBlur={(event) => {
                if (event.currentTarget.contains(event.relatedTarget as Node | null)) return
                if (!thread$.search.peek()) thread$.searchOpen.set(false)
              }}
            >
              <Search size={14} className="text-secondary shrink-0" />
              <input
                ref={desktopSearchInputRef}
                value={threadSearch}
                onChange={(event) => thread$.search.set(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault()
                    goToSearchMatch(event.shiftKey ? -1 : 1)
                  }
                  if (event.key === 'Escape') {
                    thread$.search.set('')
                    thread$.searchOpen.set(false)
                  }
                }}
                placeholder={t('chat.searchThread')}
                className="min-w-0 flex-1 bg-transparent text-xs text-primary placeholder-secondary border-none outline-none"
              />
              <button
                onClick={() => {
                  thread$.search.set('')
                  thread$.searchOpen.set(false)
                }}
                className="flex h-5 w-5 items-center justify-center rounded-full text-secondary hover:text-primary cursor-pointer"
                title={t('chat.closeThreadSearch')}
              >
                <X size={12} />
              </button>
              <span className="w-10 text-center text-[0.625rem] font-semibold text-secondary">
                {normalizedThreadSearch ? `${matchCount ? activeSearchIndex + 1 : 0}/${matchCount}` : ''}
              </span>
              <button
                onClick={() => goToSearchMatch(-1)}
                disabled={matchCount === 0}
                className="flex h-6 w-6 items-center justify-center rounded-lg text-secondary hover:bg-active disabled:opacity-35 disabled:cursor-not-allowed cursor-pointer"
                title={t('chat.previousMatch')}
              >
                <ChevronUp size={14} />
              </button>
              <button
                onClick={() => goToSearchMatch(1)}
                disabled={matchCount === 0}
                className="flex h-6 w-6 items-center justify-center rounded-lg text-secondary hover:bg-active disabled:opacity-35 disabled:cursor-not-allowed cursor-pointer"
                title={t('chat.nextMatch')}
              >
                <ChevronDown size={14} />
              </button>
            </div>
          )}
          <IconButton
            icon={PanelRight}
            label={isRSS ? t('chat.feedDetails') : t('chat.conversationDetails')}
            active={mediaOpen}
            aria-expanded={mediaOpen}
            onClick={() => thread$.mediaOpen.set(!mediaOpen)}
          />
          <div ref={actionsMenuRef} className="relative">
            <IconButton
              icon={MoreHorizontal}
              label={t('chat.moreActions')}
              active={actionsMenuOpen}
              onClick={() => setActionsMenuOpen((open) => !open)}
            />
            {actionsMenuOpen && (
              <MenuSurface className="absolute right-0 top-full z-50 mt-2 w-52 shadow-xl">{actionItems}</MenuSurface>
            )}
          </div>
        </div>
      </header>
      {viewActive && conversationMenu && (
        <FloatingContextMenu
          x={conversationMenu.x}
          y={conversationMenu.y}
          onClose={() => thread$.conversationMenu.set(null)}
          overlay
          overlayClassName="fixed inset-0 z-[60]"
          className="fixed z-[61] w-52 shadow-xl"
          onContextMenu={(event) => event.preventDefault()}
        >
          {actionItems}
        </FloatingContextMenu>
      )}
    </>
  )
}
import { MenuSurface } from '../menu/MenuSurface'
