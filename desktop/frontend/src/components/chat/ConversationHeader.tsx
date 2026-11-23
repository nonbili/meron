import { useConversationView } from './conversationView'
import { useEffect, useId, useRef, useState } from 'react'
import type { RefObject } from 'react'
import {
  ChevronDown,
  ChevronLeft,
  ChevronUp,
  ChevronsDownUp,
  ChevronsUpDown,
  Code,
  FileText,
  LoaderCircle,
  MoreHorizontal,
  PanelRight,
  Reply,
  ReplyAll,
  Search,
  X,
} from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { threadWindowSession } from '../../lib/windowSession'
import { useTranslation } from '../../lib/i18n'
import { ui$ } from '../../states/ui'
import { settings$ } from '../../states/settings'
import { goToSearchMatch, setAllMessagesExpanded } from '../../states/thread'
import { accounts$, isLocalAccountId } from '../../states/accounts'
import { isRssAccount } from '../../lib/threadActions'
import { closeCurrentConversation, kanban$ } from '../../states/kanban'
import { openReplyInFullEditor } from '../../states/compose'
import { canReplyAllToThread } from '../../states/composeReply'
import { compose$ } from '../../states/composeState'
import type { Message } from '../../types'
import type { Participant } from './ConversationDetailsPanel'
import { IconButton } from '../button/IconButton'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import { targetWithin } from '../menu/useDismissOnOutside'
import { AddToTasksButton } from '../tasks/AddToTasksButton'
import { ThreadMoveCopyItems } from '../threads/ThreadMoveCopyItems'
import { ThreadTagItem } from '../threads/ThreadTagItem'
import { useConversationMode } from './useMessageView'
import { TOOLBAR_TOP_PX } from './conversationScroll'
import { useConversationActions } from './conversationActions'
import type { ConversationToolbarAction } from '../../states/settings'

// The only toggle among the actions is the star.
const STARRED_ICON = 'fill-amber-500 text-amber-500'

// A floating pill of buttons, drawn like the reader tab's.
const PILL =
  'pointer-events-auto flex shrink-0 items-center gap-0.5 rounded-full border border-border/40 bg-chats p-1 shadow-float'

// The conversation toolbar, two pills floating over the top of the message
// list: back/close affordances and the actions pinned in Settings on the left;
// the desktop in-thread search box, the details toggle and the overflow actions
// menu on the right. The list's top padding clears them (conversationScroll).
// The subject heads the message list (ConversationSubject) and scrolls away
// with it; hovering the details button previews it, with who is in the
// conversation, wherever the list is scrolled. Search match state is published
// to thread$ by useThreadSearch.
export function ConversationHeader({
  activeThread,
  desktopSearchInputRef,
  participants,
  messageCount,
}: {
  activeThread: Message
  desktopSearchInputRef: RefObject<HTMLInputElement | null>
  participants: Participant[]
  messageCount: number
}) {
  const { state: thread$, active: viewActive, scope } = useConversationView()
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
  const normalizedThreadSearch = threadSearch.trim().toLowerCase()

  const detailsPreviewId = useId()
  const detailsLabel = isRSS ? t('chat.feedDetails') : t('chat.conversationDetails')
  const { actions, printing } = useConversationActions(activeThread, scope)
  const pinned = useValue(settings$.conversationToolbar).filter((id) => actions[id].available)
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

  // Hidden when the conversation's reply target has no other recipients: a
  // reply-all identical to the reply is a second name for the same action.
  const replyAll = useValue(canReplyAllToThread)

  // The left pill: back on a narrow window, close on a board, and the actions
  // pinned in Settings. A wide window may leave it with none of them.
  const hasDesktopLeftItems = showCloseConversation || printing || pinned.length > 0
  const hasLeftItems = hasDesktopLeftItems || !threadWindowSession
  const closeActionMenus = () => {
    setActionsMenuOpen(false)
    thread$.conversationMenu.set(null)
  }
  // A shared action as a row of the menu below.
  const menuAction = (id: ConversationToolbarAction) => {
    const action = actions[id]
    if (!action.available) return null
    return (
      <MenuItem
        icon={<action.icon size={15} className={`shrink-0 ${action.active ? STARRED_ICON : ''}`} />}
        label={action.label}
        danger={action.danger}
        disabled={action.disabled}
        className="h-auto! gap-2.5! px-3! py-2 text-xs! font-medium! disabled:opacity-50 disabled:cursor-default disabled:hover:bg-transparent"
        onClick={() => {
          closeActionMenus()
          action.run()
        }}
      />
    )
  }
  // The thread's actions, shown both under the header's button and, for a
  // right-click on the blank part of the conversation, at the pointer.
  const actionItems = (
    <>
      {menuAction('openInNewWindow')}

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
      {menuAction('star')}
      {!isRSS && (
        <>
          <ThreadTagItem
            key={activeThread.thread_id}
            threadId={activeThread.thread_id}
            close={closeActionMenus}
            itemClassName="h-auto! gap-2.5! px-3! py-2 text-xs! font-medium!"
            iconSize={15}
          />
          {menuAction('archive')}
          {menuAction('spam')}
          <ThreadMoveCopyItems
            threadId={activeThread.thread_id}
            accountId={activeThread.account_id}
            folderId={activeThread.folder_id}
            close={closeActionMenus}
            itemClassName="h-auto! gap-2.5! px-3! py-2 text-xs! font-medium!"
            iconSize={15}
          />
          <div className="my-1 h-px bg-border" />
          {menuAction('print')}
          {menuAction('saveEml')}
          <div className="my-1 h-px bg-border" />
          {menuAction('delete')}
        </>
      )}
    </>
  )

  return (
    <>
      <div
        className="pointer-events-none absolute inset-x-0 top-0 z-40 flex items-start justify-between gap-2 pl-3 pr-5 select-none"
        style={{ paddingTop: TOOLBAR_TOP_PX }}
      >
        <div className={`${PILL} ${hasDesktopLeftItems ? '' : 'min-[769px]:hidden'} ${hasLeftItems ? '' : 'hidden'}`}>
          {!threadWindowSession && (
            <IconButton
              icon={ChevronLeft}
              iconSize={20}
              label={t('chat.backToChats')}
              className="min-[769px]:hidden"
              onClick={() => ui$.mobilePane.set('threads')}
            />
          )}
          {showCloseConversation && (
            <IconButton
              icon={X}
              iconSize={18}
              label={t('chat.closeConversationEsc')}
              className="max-[768px]:hidden"
              onClick={closeCurrentConversation}
            />
          )}
          {pinned.map((id) => {
            const action = actions[id]
            if (id === 'tasks')
              return (
                <AddToTasksButton
                  key={id}
                  title={activeThread.subject}
                  account={activeThread.account_id}
                  threadId={activeThread.thread_id}
                />
              )
            return (
              <IconButton
                key={id}
                label={action.label}
                disabled={action.disabled}
                aria-pressed={id === 'star' ? !!action.active : undefined}
                onClick={action.run}
              >
                <action.icon size={16} className={action.active ? STARRED_ICON : ''} />
              </IconButton>
            )
          })}
          {printing && (
            <span className="flex h-9 w-9 items-center justify-center">
              <LoaderCircle
                size={16}
                className="animate-spin text-secondary"
                role="status"
                aria-label={t('chat.actions.printThread')}
              />
            </span>
          )}
        </div>

        {/* ml-auto keeps it right when the left pill is hidden. */}
        <div className={`${PILL} ml-auto`}>
          {!threadSearchOpen ? (
            <IconButton icon={Search} label={t('chat.searchThread')} onClick={() => thread$.searchOpen.set(true)} />
          ) : (
            <div
              className="hidden min-[900px]:flex w-[286px] items-center gap-1 rounded-full bg-hover px-2.5 py-1.5 border border-transparent focus-within:border-accent/40 focus-within:bg-chats"
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
          <div className="group/details relative">
            <IconButton
              icon={PanelRight}
              label={detailsLabel}
              active={mediaOpen}
              aria-expanded={mediaOpen}
              aria-describedby={detailsPreviewId}
              // The preview below stands in for the native tooltip.
              title={mediaOpen ? detailsLabel : ''}
              onClick={() => thread$.mediaOpen.set(!mediaOpen)}
            />
            {/* A preview of what the button opens: the subject, which has
                usually scrolled away above, and who is in the conversation.
                Not while the panel itself is open. */}
            {!mediaOpen && (
              <ConversationPreview
                id={detailsPreviewId}
                subject={activeThread.subject}
                feedHost={isRSS ? activeThread.from_addr : ''}
                participants={participants}
                messageCount={messageCount}
              />
            )}
          </div>
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
      </div>
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

// How many names the preview lists before summing up the rest as a count.
const PREVIEW_NAMES = 3

function ConversationPreview({
  id,
  subject,
  feedHost,
  participants,
  messageCount,
}: {
  id: string
  subject: string
  /** An RSS item's feed host, in place of the people. */
  feedHost: string
  participants: Participant[]
  messageCount: number
}) {
  const { t } = useTranslation()
  // Others first (buildParticipants keeps "you" out of the lead), then you.
  const names = participants.slice(0, PREVIEW_NAMES).map((person) => person.name)
  const counts = [
    messageCount > 0 ? t('chat.messagesCount', { count: messageCount }) : '',
    participants.length > PREVIEW_NAMES ? t('chat.people', { count: participants.length }) : '',
  ].filter(Boolean)
  return (
    <span
      id={id}
      role="tooltip"
      // Shown after a short pause, so passing over the button on the way
      // elsewhere doesn't flash it; hidden at once on leaving.
      className="pointer-events-none invisible absolute right-0 top-[calc(100%+0.75rem)] z-50 w-max max-w-72 rounded-xl border border-border bg-chats px-3 py-2 text-left opacity-0 shadow-lg transition-[opacity,visibility] group-hover/details:visible group-hover/details:opacity-100 group-hover/details:delay-300 group-has-[:focus-visible]/details:visible group-has-[:focus-visible]/details:opacity-100"
    >
      <span className="block break-words text-xs font-semibold leading-snug text-primary">
        {subject || '(no subject)'}
      </span>
      {feedHost ? (
        <span className="mt-1 block truncate text-[0.6875rem] text-secondary">{feedHost}</span>
      ) : (
        names.length > 0 && (
          <span className="mt-1 block truncate text-[0.6875rem] text-secondary">{names.join(', ')}</span>
        )
      )}
      {counts.length > 0 && <span className="mt-0.5 block text-[0.6875rem] text-secondary">{counts.join(' · ')}</span>}
    </span>
  )
}
