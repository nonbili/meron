import { useEffect, useRef, useState } from 'react'
import type { MouseEvent as ReactMouseEvent, RefObject } from 'react'
import {
  Archive,
  ChevronDown,
  ChevronLeft,
  ChevronUp,
  Code,
  Copy,
  FileText,
  Mail,
  MoreVertical,
  PanelRight,
  Search,
  SquarePen,
  Star,
  Trash2,
  X,
} from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { showToast, ui$ } from '../../states/ui'
import { archiveThread, deleteThread, starThread } from '../../states/mail'
import { thread$, type ConversationMode } from '../../states/thread'
import { closeKanbanPane, kanban$ } from '../../states/kanban'
import { openComposeTab } from '../../states/compose'
import type { Message } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { IconButton } from '../button/IconButton'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import { ConversationSubject } from './ConversationSubject'

// The conversation header: back/close affordances, sender info, the desktop
// in-thread search box and the overflow actions menu (view mode, star, archive,
// delete). Search match state is computed by the parent via useThreadSearch.
export function ConversationHeader({
  activeThread,
  isRSS,
  conversationMode,
  setQuickConversationMode,
  searchMatches,
  activeSearchIndex,
  goToSearchMatch,
  desktopSearchInputRef,
}: {
  activeThread: Message
  isRSS: boolean
  conversationMode: ConversationMode
  setQuickConversationMode: (mode: ConversationMode) => void
  searchMatches: string[]
  activeSearchIndex: number
  goToSearchMatch: (direction: -1 | 1) => void
  desktopSearchInputRef: RefObject<HTMLInputElement | null>
}) {
  const { t } = useTranslation()
  const inKanban = !!useValue(kanban$.activeBoardId)
  const threadSearch = useValue(thread$.search)
  const threadSearchOpen = useValue(thread$.searchOpen)
  const mediaOpen = useValue(thread$.mediaOpen)
  const normalizedThreadSearch = threadSearch.trim().toLowerCase()

  const [actionsMenuOpen, setActionsMenuOpen] = useState(false)
  const [senderMenu, setSenderMenu] = useState<{ x: number; y: number } | null>(null)
  const actionsMenuRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    if (!actionsMenuOpen) return
    const onDown = (event: MouseEvent) => {
      if (actionsMenuRef.current && !actionsMenuRef.current.contains(event.target as Node)) {
        setActionsMenuOpen(false)
      }
    }
    window.addEventListener('mousedown', onDown)
    return () => window.removeEventListener('mousedown', onDown)
  }, [actionsMenuOpen])

  useEffect(() => {
    setSenderMenu(null)
  }, [activeThread.thread_id])

  const copyHeaderText = (text: string, toast: string) => {
    const value = text.trim()
    if (!value) return
    const write = navigator.clipboard?.writeText(value)
    if (!write) return
    write
      .then(() => {
        showToast(toast)
      })
      .catch(() => undefined)
  }

  const senderName = activeThread.from_name.trim()
  const senderEmail = activeThread.from_addr.trim()
  const senderDisplayName = senderName || senderEmail
  const senderRecipient = senderName && senderName !== senderEmail ? `${senderName} <${senderEmail}>` : senderEmail

  const openSenderMenu = (event: ReactMouseEvent<HTMLElement>) => {
    event.preventDefault()
    setSenderMenu({ x: event.clientX, y: event.clientY })
  }

  return (
    <>
      <header className="relative z-40 flex h-16 shrink-0 items-center gap-3 border-b border-border bg-header px-2 select-none">
        <button
          className="flex h-8 w-8 items-center justify-center rounded-full hover:bg-hover text-secondary min-[769px]:hidden cursor-pointer"
          onClick={() => ui$.mobilePane.set('threads')}
          title={t('chat.backToChats')}
        >
          <ChevronLeft size={20} />
        </button>

        {inKanban && (
          <button
            className="-mr-2 flex h-8 w-8 shrink-0 items-center justify-center rounded-full hover:bg-hover text-secondary cursor-pointer max-[768px]:hidden"
            onClick={closeKanbanPane}
            title={t('chat.closeConversationEsc')}
          >
            <X size={18} />
          </button>
        )}

        <Avatar
          name={activeThread.from_name || activeThread.from_addr}
          email={isRSS ? undefined : activeThread.from_addr}
          src={isRSS && activeThread.feed_icon ? `/media/${activeThread.feed_icon}` : undefined}
        />

        <div className="min-w-0 flex-1">
          <ConversationSubject
            subject={activeThread.subject}
            copyLabel={t('chat.copySubject')}
            onCopy={() => copyHeaderText(activeThread.subject, 'Subject copied')}
          />
          {isRSS ? (
            // RSS subject == from_name (both the feed title), so showing the name
            // again would just duplicate the title above. Show the feed host only.
            <p className="truncate text-[11.5px] text-secondary mt-0.5 font-medium" title={activeThread.from_addr}>
              {activeThread.from_addr}
            </p>
          ) : (
            <button
              type="button"
              onClick={openSenderMenu}
              onContextMenu={openSenderMenu}
              className="mt-0.5 block max-w-full truncate rounded-sm text-left text-[11.5px] font-medium text-secondary outline-none transition-colors cursor-context-menu hover:text-accent focus-visible:ring-2 focus-visible:ring-accent/40"
              title={`${activeThread.from_name} (${activeThread.from_addr})`}
              aria-label={t('chat.copyFullAddress')}
            >
              {senderName ? (
                <>
                  {senderName} <span className="opacity-70">({senderEmail})</span>
                </>
              ) : (
                senderEmail
              )}
            </button>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {!threadSearchOpen ? (
            <IconButton icon={Search} label={t('chat.searchThread')} onClick={() => thread$.searchOpen.set(true)} />
          ) : (
            <div className="hidden min-[900px]:flex w-[286px] items-center gap-1 rounded-xl bg-hover px-2 py-1.5 border border-transparent focus-within:border-accent/40 focus-within:bg-chats">
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
              <span className="w-10 text-center text-[10px] font-semibold text-secondary">
                {normalizedThreadSearch
                  ? `${searchMatches.length ? activeSearchIndex + 1 : 0}/${searchMatches.length}`
                  : ''}
              </span>
              <button
                onClick={() => goToSearchMatch(-1)}
                disabled={searchMatches.length === 0}
                className="flex h-6 w-6 items-center justify-center rounded-lg text-secondary hover:bg-active disabled:opacity-35 disabled:cursor-not-allowed cursor-pointer"
                title={t('chat.previousMatch')}
              >
                <ChevronUp size={14} />
              </button>
              <button
                onClick={() => goToSearchMatch(1)}
                disabled={searchMatches.length === 0}
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
            onClick={() => thread$.mediaOpen.set(!mediaOpen)}
          />
          <div ref={actionsMenuRef} className="relative">
            <IconButton
              icon={MoreVertical}
              label={t('chat.moreActions')}
              active={actionsMenuOpen}
              onClick={() => setActionsMenuOpen((open) => !open)}
            />
            {actionsMenuOpen && (
              <div className="absolute right-0 top-full z-50 mt-2 w-52 rounded-xl border border-border bg-chats p-1 shadow-xl">
                <button
                  onClick={() => {
                    setQuickConversationMode('html')
                    setActionsMenuOpen(false)
                  }}
                  className={`flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs cursor-pointer hover:bg-hover ${
                    conversationMode === 'html' ? 'font-semibold text-accent' : 'font-medium text-primary'
                  }`}
                >
                  <Code size={15} className="shrink-0" /> {t('chat.viewAsHtml')}
                </button>
                <button
                  onClick={() => {
                    setQuickConversationMode('plain')
                    setActionsMenuOpen(false)
                  }}
                  className={`flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs cursor-pointer hover:bg-hover ${
                    conversationMode === 'plain' ? 'font-semibold text-accent' : 'font-medium text-primary'
                  }`}
                >
                  <FileText size={15} className="shrink-0" /> {t('chat.viewAsPlainText')}
                </button>
                <div className="my-1 h-px bg-border" />
                <button
                  onClick={() => {
                    void starThread(activeThread.thread_id, !activeThread.starred)
                    setActionsMenuOpen(false)
                  }}
                  className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-primary cursor-pointer hover:bg-hover"
                >
                  <Star
                    size={15}
                    className={`shrink-0 ${activeThread.starred ? 'fill-amber-500 text-amber-500' : ''}`}
                  />
                  {activeThread.starred ? t('chat.unstar') : t('chat.star')}
                </button>
                {!isRSS && (
                  <>
                    <button
                      onClick={() => {
                        void archiveThread(activeThread.thread_id)
                        setActionsMenuOpen(false)
                      }}
                      className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-primary cursor-pointer hover:bg-hover"
                    >
                      <Archive size={15} className="shrink-0" /> {t('threads.actions.archiveThread')}
                    </button>
                    <button
                      onClick={() => {
                        void deleteThread(activeThread.thread_id)
                        setActionsMenuOpen(false)
                      }}
                      className="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-rose-600 dark:text-rose-400 cursor-pointer hover:bg-rose-50 dark:hover:bg-rose-950/25"
                    >
                      <Trash2 size={15} className="shrink-0" /> {t('threads.actions.moveToTrash')}
                    </button>
                  </>
                )}
              </div>
            )}
          </div>
        </div>
      </header>
      {senderMenu && !isRSS && (
        <FloatingContextMenu
          x={senderMenu.x}
          y={senderMenu.y}
          onClose={() => setSenderMenu(null)}
          overlay
          overlayClassName="fixed inset-0 z-[60]"
          className="fixed z-[61] min-w-[180px] rounded-xl border border-border bg-header p-1 shadow-xl"
          onContextMenu={(event) => event.preventDefault()}
        >
          <MenuItem
            icon={<Copy size={13} className="text-accent" />}
            label={t('chat.copyName', { defaultValue: 'Copy name' })}
            disabled={!senderDisplayName}
            onClick={() => {
              copyHeaderText(senderDisplayName, 'Name copied')
              setSenderMenu(null)
            }}
          />
          <MenuItem
            icon={<Mail size={13} className="text-accent" />}
            label={t('chat.copyEmailAddress')}
            disabled={!senderEmail}
            onClick={() => {
              copyHeaderText(senderEmail, 'Email copied')
              setSenderMenu(null)
            }}
          />
          <MenuItem
            icon={<SquarePen size={13} className="text-accent" />}
            label={t('chat.newMessageTo', { email: senderEmail })}
            disabled={!senderEmail}
            onClick={() => {
              openComposeTab({ accountId: activeThread.account_id, to: senderRecipient })
              setSenderMenu(null)
            }}
          />
        </FloatingContextMenu>
      )}
    </>
  )
}
