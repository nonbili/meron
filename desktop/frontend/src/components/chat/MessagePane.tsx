import { retainThreadView } from '../../states/threadView'
import type { RefObject } from 'react'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openComposeTab } from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { withoutHydratedQuickReplyDraft } from '../../states/quickReply'
import { ui$ } from '../../states/ui'
import { findLocalThread, loadThread, threadFromTab } from '../../states/mail'
import { accounts$, isLocalAccountId } from '../../states/accounts'
import { settings$ } from '../../states/settings'
import { thread$ } from '../../states/thread'
import { ensureThreadData, retainThreadData, threadData$ } from '../../states/threadData'
import { ConversationViewContext, useConversationState, useConversationView } from './conversationView'
import { Gallery } from './Gallery'
import { ConversationDetailsPanel } from './ConversationDetailsPanel'
import { EmptyState } from '../empty-state/EmptyState'
import { QuickReplyComposer } from './QuickReplyComposer'
import { addQuickReplyAttachments } from './useQuickReply'
import { DropOverlay } from './DropOverlay'
import { useFileDrop } from '../../lib/useFileDrop'
import { ConversationTabs } from './ConversationTabs'
import { ReaderTabView } from './ReaderTabView'
import { ConversationHeader } from './ConversationHeader'
import { ThreadSearchBarMobile } from './ThreadSearchBarMobile'
import { ConversationMessageList } from './ConversationMessageList'
import { MessageContextMenu } from './MessageContextMenu'
import { buildGalleryItems, buildThreadMedia, buildParticipants } from './conversationMedia'
import { useThreadSearch } from './useThreadSearch'
import { FrostedBand } from './FrostedBand'
import {
  ConversationScrollContext,
  ConversationScrollIntentContext,
  useConversationScroll,
} from './useConversationScroll'
import { COMPOSER_OVERLAP_VAR } from './conversationScroll'
import { ArrowRight } from 'lucide-react'
import { usePresence } from '../../lib/usePresence'
import { PANE_ANIMATION_MS } from '../kanban/KanbanConversationPane'
import { wallpaperCss } from '../../lib/wallpapers'
import { clearMediaSession } from '../../lib/mediaSession'
import { openCorrespondentMail } from '../../states/kanban'
import { isRssAccount } from '../../lib/threadActions'
import type { Message } from '../../types'

export function MessagePane() {
  const tabs = useValue(compose$.tabs)
  const activeTab = useValue(compose$.activeTab)
  const currentThread = useValue(compose$.conversationThread)
  const selectedThread = useValue(ui$.selectedThread)
  const mobilePane = useValue(ui$.mobilePane)
  const accounts = useValue(accounts$)
  const selectedAccount = useValue(ui$.selectedAccount)
  const activeTabData = tabs.find((tab) => tab.id === activeTab)
  const documentTab = activeTabData?.kind === 'reader' || activeTabData?.kind === 'compose' ? activeTabData : null
  const currentId = activeTab === '' ? selectedThread : currentThread
  const current = useValue(() =>
    currentId ? (findLocalThread(currentId) ?? threadData$[currentId].messages.get()?.[0]) : null,
  )
  const activeThread =
    activeTabData?.kind === 'thread'
      ? threadFromTab(activeTabData, null)
      : documentTab
        ? (findLocalThread(selectedThread) ?? null)
        : current
  const accountId = activeThread?.account_id ?? selectedAccount
  const account = accounts.find((item) => item.id === accountId)
  const showComposer = !!activeThread && !isRssAccount(account, accountId) && !isLocalAccountId(accountId)
  const composerRef = useRef<HTMLDivElement | null>(null)
  const {
    dragging: fileDragging,
    mayBeLink,
    dropHandlers,
  } = useFileDrop(addQuickReplyAttachments, {
    enabled: showComposer && !documentTab,
    scope: selectedThread,
  })

  // Menus and lightboxes are transient interactions. They belong only to the
  // foreground tab, unlike the mounted conversation's search and message bodies.
  useLayoutEffect(() => {
    thread$.galleryIndex.set(null)
    thread$.mediaOpen.set(false)
    thread$.hoveredLink.set(null)
    thread$.messageContextMenu.set(null)
    thread$.conversationMenu.set(null)
  }, [activeTab, selectedThread])

  return (
    <div
      className={`relative flex flex-1 flex-col overflow-hidden ${mobilePane === 'conversation' ? 'max-[768px]:flex' : 'max-[768px]:hidden'}`}
    >
      <ConversationTabs />
      <div
        className={`relative flex min-h-0 flex-1 flex-col overflow-hidden ${fileDragging && mayBeLink ? '[&_iframe]:pointer-events-none' : ''}`}
        {...dropHandlers}
      >
        <KeptConversation
          key={`current:${currentId}`}
          scope={currentId}
          active={activeTab === ''}
          thread={current ?? null}
          composerRef={composerRef}
        />
        {tabs
          .filter((tab) => tab.kind === 'thread')
          .map((tab) => (
            <KeptConversation
              key={tab.id}
              tabOwned
              scope={`${tab.id}:${tab.threadId}`}
              active={activeTab === tab.id}
              thread={threadFromTab(tab, findLocalThread(tab.threadId) ?? null)}
              composerRef={composerRef}
            />
          ))}
        {showComposer && (
          <div
            ref={composerRef}
            aria-hidden={!!documentTab}
            inert={!!documentTab}
            className={`pointer-events-none absolute inset-x-0 bottom-0 z-10 ${documentTab ? 'invisible' : ''}`}
          >
            <FrostedBand fade="top" className="bottom-0 left-0 right-3 top-5" />
            <div className="relative">
              <ConversationViewContext value={{ state: thread$, active: !documentTab, scope: selectedThread }}>
                <QuickReplyComposer />
              </ConversationViewContext>
            </div>
          </div>
        )}
        {fileDragging && <DropOverlay passThrough={mayBeLink} />}
        {documentTab && (
          <div className="absolute inset-0 z-20 flex flex-col bg-chat">
            <ReaderTabView tab={documentTab} />
          </div>
        )}
      </div>
    </div>
  )
}

function KeptConversation({
  active,
  thread,
  scope,
  composerRef,
  tabOwned = false,
}: {
  tabOwned?: boolean
  active: boolean
  thread: Message | null
  scope: string
  composerRef: RefObject<HTMLDivElement | null>
}) {
  useLayoutEffect(() => (scope ? retainThreadView(scope, tabOwned) : undefined), [scope, tabOwned])
  const state = useConversationState(active)
  const value = useMemo(() => ({ state, active, scope }), [state, active, scope])
  return (
    <ConversationViewContext value={value}>
      <div
        aria-hidden={!active}
        inert={!active}
        className={`absolute inset-0 flex flex-col ${active ? '' : 'invisible pointer-events-none'}`}
      >
        <ConversationPane activeThread={thread} composerRef={composerRef} />
      </div>
    </ConversationViewContext>
  )
}

function ConversationPane({
  activeThread,
  composerRef,
}: {
  activeThread: Message | null
  composerRef: RefObject<HTMLDivElement | null>
}) {
  const { t } = useTranslation()
  const { state: thread$, active } = useConversationView()
  const activeThreadId = activeThread?.thread_id ?? ''
  const data = ensureThreadData(activeThreadId)
  const messages = useValue(data.messages)
  const messagesCursor = useValue(data.cursor)
  const messagesLoadingMore = useValue(data.loadingMore)
  const threadLoading = useValue(data.loading)
  const threadError = useValue(data.error)
  const accounts = useValue(accounts$)
  const selectedAccountId = useValue(ui$.selectedAccount)
  const revealedRemote = useValue(thread$.revealedRemote)
  const allowedSenders = useValue(settings$.remoteImageSenders)
  const galleryIndex = useValue(thread$.galleryIndex)
  const mediaOpen = useValue(thread$.mediaOpen)
  const flashMessageId = useValue(thread$.flashMessageId)
  const quickReplyDraftId = useValue(compose$.quickReplyDraftId)
  const quickReplyDraftSaved = useValue(compose$.quickReplyDraftSaved)
  const sendingDraftIds = useValue(compose$.sendingDraftIds)
  const conversationActiveTab = active ? '' : 'hidden'

  useEffect(() => (activeThreadId ? retainThreadData(activeThreadId) : undefined), [activeThreadId])
  useEffect(() => {
    if (activeThreadId && (active || !data.messages.peek().length) && !data.loading.peek())
      void loadThread(activeThreadId)
  }, [active, activeThreadId])
  const detailsOpen = !!activeThread && active && mediaOpen
  // Slides open and shut beside the conversation on a wide window (index.css,
  // .details-slide), like the board's conversation pane.
  const detailsPhase = usePresence(detailsOpen, PANE_ANIMATION_MS)

  const activeAccount = activeThread
    ? accounts.find((account) => account.id === activeThread.account_id)
    : (accounts.find((account) => account.id === selectedAccountId) ?? null)
  const isRSS = isRssAccount(activeAccount ?? undefined, activeThread?.account_id ?? selectedAccountId)
  const conversationWallpaper = wallpaperCss(activeAccount?.chat_wallpaper)
  // Show a spinner instead of the previous thread's messages while a freshly
  // selected thread loads. Suppressed once any loaded message belongs to the
  // active thread, so re-reads of the open thread (sync refresh) don't flash.
  const showThreadLoading = threadLoading && !messages.some((message) => message.thread_id === activeThreadId)
  // The load failed and there's nothing of this thread to show — offer a retry
  // instead of a blank pane. Suppressed while a (re)load is in flight.
  const showThreadError =
    !showThreadLoading && threadError && !messages.some((message) => message.thread_id === activeThreadId)
  const retryThreadLoad = useCallback(() => {
    if (activeThreadId) void loadThread(activeThreadId).catch(console.error)
  }, [activeThreadId])
  // Keep the draft hydrated into the quick-reply bar out of the conversation,
  // including while an optimistic sending bubble follows it. Drafts a send has
  // already taken over stay hidden too, until the post-send discard settles —
  // the quick reply has let go of them by then, so nothing else would.
  const displayMessages = useMemo(
    () => withoutHydratedQuickReplyDraft(messages, quickReplyDraftId, quickReplyDraftSaved, sendingDraftIds),
    [messages, quickReplyDraftId, quickReplyDraftSaved, sendingDraftIds],
  )
  const unreadKey = displayMessages.map((message) => `${message.id}:${message.unread ? '1' : '0'}`).join('|')

  const { galleryItems, galleryOffsets } = useMemo(
    () => buildGalleryItems(displayMessages, accounts, revealedRemote, allowedSenders),
    [displayMessages, accounts, revealedRemote, allowedSenders],
  )
  const { mediaItems, fileItems } = useMemo(
    () => buildThreadMedia(displayMessages, accounts, revealedRemote, allowedSenders),
    [displayMessages, accounts, revealedRemote, allowedSenders],
  )
  const participants = useMemo(
    () => buildParticipants(displayMessages, accounts, isRSS),
    [displayMessages, accounts, isRSS],
  )

  const threadSearchFocus = useValue(thread$.searchFocus)
  const desktopThreadSearchInputRef = useRef<HTMLInputElement | null>(null)
  const mobileThreadSearchInputRef = useRef<HTMLInputElement | null>(null)
  const { threadSearchOpen, searchMatches, activeSearchId } = useThreadSearch(displayMessages, activeThread?.account_id)
  const conversationRef = useRef<HTMLDivElement | null>(null)
  const hasConversation = !!activeThread
  // The composer floats over the bottom of the message list, so messages scroll
  // on behind its rounded box instead of ending at a straight edge above it. The
  // list pads its end by the composer's height (COMPOSER_OVERLAP_VAR) so the
  // newest message still clears it. Measured ahead of useConversationScroll,
  // whose layout effects run in order after this one: its first positioning and
  // read-marking pass on a pane that mounts with its messages already loaded
  // would otherwise count the ones behind the composer as seen.
  useLayoutEffect(() => {
    const host = conversationRef.current
    const composer = composerRef.current
    if (!active || !host || !composer) return
    // The shared composer follows the message column when the details panel
    // changes its width, just as it did when it lived inside that column.
    composer.style.width = `${host.offsetWidth}px`
    host.style.setProperty(COMPOSER_OVERLAP_VAR, `${composer.offsetHeight}px`)
  }, [active, hasConversation, isRSS])
  const conversationScroll = useConversationScroll(activeThreadId, displayMessages, conversationActiveTab, unreadKey)
  const { scrollRef, setScrollTop } = conversationScroll

  const contextMenu = useValue(thread$.messageContextMenu)

  const showMessageInConversation = useCallback((messageId: string) => {
    thread$.pendingScrollMessageId.set(messageId)
    thread$.galleryIndex.set(null)
    thread$.mediaOpen.set(false)
  }, [])

  // Keeps the overlap in step as the composer grows with a longer reply or an
  // attachment, and a reader at the bottom parked there. That goes through the
  // scroll controller like any other positioning of ours, or the scroll it
  // causes would read as the reader's and drop a settling pin.
  useLayoutEffect(() => {
    const host = conversationRef.current
    const container = scrollRef.current
    const composer = composerRef.current
    if (!active || !host || !container || !composer) return
    const observer = new ResizeObserver(() => {
      const atBottom = container.scrollHeight - container.scrollTop - container.clientHeight <= 1
      composer.style.width = `${host.offsetWidth}px`
      host.style.setProperty(COMPOSER_OVERLAP_VAR, `${composer.offsetHeight}px`)
      if (atBottom) setScrollTop(container.scrollHeight)
    })
    observer.observe(composer)
    observer.observe(host)
    return () => observer.disconnect()
  }, [active, hasConversation, isRSS, scrollRef, setScrollTop])

  useLayoutEffect(() => {
    return () => {
      const container = scrollRef.current
      if (container) {
        container.querySelectorAll<HTMLMediaElement>('video, audio').forEach((media) => {
          try {
            media.pause()
            media.querySelectorAll('source').forEach((source) => source.remove())
            media.removeAttribute('src')
            media.load()
          } catch {
            // Ignore
          }
        })

        container.querySelectorAll('iframe').forEach((iframe) => {
          try {
            const doc = iframe.contentDocument
            const win = iframe.contentWindow
            if (doc) {
              doc.querySelectorAll<HTMLMediaElement>('video, audio').forEach((media) => {
                try {
                  media.pause()
                  media.querySelectorAll('source').forEach((source) => source.remove())
                  media.removeAttribute('src')
                  media.load()
                } catch {
                  // Ignore
                }
              })
              doc.querySelectorAll('iframe').forEach((nestedIframe) => {
                try {
                  nestedIframe.removeAttribute('src')
                  nestedIframe.src = 'about:blank'
                  nestedIframe.remove()
                } catch {
                  // Ignore
                }
              })
            }
            if (win) {
              clearMediaSession(win)
            }
            iframe.removeAttribute('src')
            iframe.src = 'about:blank'
          } catch {
            // Ignore
          }
        })
      }
      clearMediaSession()
    }
  }, [activeThreadId])

  useEffect(() => {
    if (!active || !threadSearchOpen) return
    // threadSearchFocus also re-runs this when ⌘/Ctrl+F is pressed with the bar
    // already open — including from inside an HTML message frame, which is
    // where focus otherwise stays.
    const frame = window.requestAnimationFrame(() => {
      const input = window.matchMedia('(min-width: 900px)').matches
        ? desktopThreadSearchInputRef.current
        : mobileThreadSearchInputRef.current
      input?.focus()
    })
    return () => window.cancelAnimationFrame(frame)
  }, [active, threadSearchOpen, threadSearchFocus])

  // Bring the message holding the active match into view — which also mounts an
  // HTML body's frame. Each body then scrolls to the exact <mark> it rendered,
  // including when the search moves between two matches in the same message,
  // which is why the offset is deliberately not a dependency here.
  useEffect(() => {
    if (!active || !activeSearchId) return
    const container = scrollRef.current
    const match = container?.querySelector<HTMLElement>(`[data-search-match-id="${CSS.escape(activeSearchId)}"]`)
    match?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }, [activeSearchId])

  const conversationContent = activeThread ? (
    <>
      {activeThread.original_thread_id && (
        <div className="flex shrink-0 items-center justify-between bg-accent/10 dark:bg-accent/15 px-4 py-2.5 text-xs border-b border-border select-none transition-colors">
          <span className="text-secondary/90 font-medium">{t('chat.branchNotice')}</span>
          <button
            onClick={() => ui$.selectedThread.set(activeThread.original_thread_id!)}
            className="flex items-center gap-1 font-semibold text-accent hover:underline cursor-pointer"
          >
            <span>{t('chat.viewOriginalTopic')}</span>
            <ArrowRight size={13} />
          </button>
        </div>
      )}

      {threadSearchOpen && <ThreadSearchBarMobile inputRef={mobileThreadSearchInputRef} />}

      <ConversationScrollContext value={conversationScroll}>
        <ConversationScrollIntentContext value={conversationScroll.releasePinForUserScroll}>
          <ConversationMessageList
            messages={displayMessages}
            showThreadLoading={showThreadLoading}
            showThreadError={showThreadError}
            onRetryThreadLoad={retryThreadLoad}
            messagesCursor={messagesCursor}
            messagesLoadingMore={messagesLoadingMore}
            activeThreadId={activeThreadId}
            searchMatches={searchMatches}
            activeSearchId={activeSearchId}
            jumpMessageId={flashMessageId}
            galleryOffsets={galleryOffsets}
            wallpaperClassName={conversationWallpaper.className}
            wallpaperStyle={conversationWallpaper.style}
          />
        </ConversationScrollIntentContext>
      </ConversationScrollContext>
    </>
  ) : (
    <div
      style={conversationWallpaper.style}
      className={`flex-1 flex items-center justify-center p-6 ${conversationWallpaper.className}`}
    >
      <EmptyState title={t('empty.noConversationSelected')} text={t('empty.noConversationSelectedText')} />
    </div>
  )

  return (
    <div className="relative flex min-h-0 flex-1 flex-col overflow-hidden">
      {activeThread && (
        <ConversationHeader activeThread={activeThread} desktopSearchInputRef={desktopThreadSearchInputRef} />
      )}
      <div className="relative flex min-h-0 flex-1 overflow-hidden">
        <section className="relative flex flex-1 flex-col overflow-hidden bg-chat">
          <div ref={conversationRef} className="relative flex min-h-0 flex-1 flex-col">
            {conversationContent}
          </div>
        </section>

        {activeThread && detailsPhase !== 'closed' && (
          <div
            data-pane-phase={detailsPhase}
            className={`details-slide${
              detailsPhase === 'entering'
                ? ' animate-pane-open'
                : detailsPhase === 'exiting'
                  ? ' animate-pane-close'
                  : ''
            }`}
          >
            <ConversationDetailsPanel
              media={mediaItems}
              files={fileItems}
              participants={participants}
              scopeTitle={activeThread.subject || '(no subject)'}
              // For RSS, from_name duplicates the subject (both the feed title), so
              // show the feed host instead; otherwise show the sender name.
              scopeSubtitle={isRSS ? activeThread.from_addr : activeThread.from_name || activeThread.from_addr}
              loading={showThreadLoading}
              onOpenImage={(index) => thread$.galleryIndex.set(index)}
              onShowInConversation={showMessageInConversation}
              onComposeTo={(person) =>
                openComposeTab({
                  accountId: activeThread.account_id,
                  to: person.name && person.name !== person.email ? `${person.name} <${person.email}>` : person.email,
                })
              }
              onViewMessagesWith={(person) =>
                openCorrespondentMail(activeThread.account_id, activeThread.folder_id, person.email)
              }
              onClose={() => thread$.mediaOpen.set(false)}
              closing={detailsPhase === 'exiting'}
            />
          </div>
        )}
      </div>

      {active && galleryIndex !== null && galleryItems[galleryIndex] && (
        <Gallery
          items={galleryItems}
          index={galleryIndex}
          onIndexChange={(idx) => thread$.galleryIndex.set(idx)}
          onClose={() => thread$.galleryIndex.set(null)}
        />
      )}

      {active && contextMenu && (
        <MessageContextMenu state={contextMenu} isRSS={isRSS} onClose={() => thread$.messageContextMenu.set(null)} />
      )}
    </div>
  )
}
