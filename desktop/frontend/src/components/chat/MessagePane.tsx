import { useCallback, useEffect, useLayoutEffect, useMemo, useRef } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openComposeTab } from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { withoutHydratedQuickReplyDraft } from '../../states/quickReply'
import { ui$ } from '../../states/ui'
import { mail$, getActiveThread, loadThread } from '../../states/mail'
import { accounts$ } from '../../states/accounts'
import { settings$ } from '../../states/settings'
import { resetThreadView, thread$ } from '../../states/thread'
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
import type { Message, MessageTab } from '../../types'

function threadFromTab(tab: MessageTab, fallback: Message | null): Message {
  if (fallback?.thread_id === tab.threadId) return fallback
  return {
    id: tab.messageId || tab.threadId,
    account_id: tab.accountId ?? '',
    folder_id: tab.folderId ?? 'inbox',
    thread_id: tab.threadId,
    from_name: '',
    from_addr: tab.from,
    to: '',
    subject: tab.subject || '(no subject)',
    preview: '',
    body: '',
    date: tab.date ?? 0,
    unread: false,
    starred: false,
    has_attachments: false,
  }
}

export function MessagePane() {
  const { t } = useTranslation()
  const messages = useValue(mail$.messages)
  const messagesCursor = useValue(mail$.messagesCursor)
  const messagesLoadingMore = useValue(mail$.messagesLoadingMore)
  const threadLoading = useValue(mail$.threadLoading)
  const threadErrorId = useValue(mail$.threadErrorId)
  const accounts = useValue(accounts$)
  const mobilePane = useValue(ui$.mobilePane)
  const selectedAccountId = useValue(ui$.selectedAccount)
  const selectedThreadId = useValue(ui$.selectedThread)
  const revealedRemote = useValue(thread$.revealedRemote)
  const allowedSenders = useValue(settings$.remoteImageSenders)
  const galleryIndex = useValue(thread$.galleryIndex)
  const mediaOpen = useValue(thread$.mediaOpen)
  // Ring-highlight for a message jumped to from the starred list; shares the
  // search-match ring styling in ConversationMessageList.
  const flashMessageId = useValue(thread$.flashMessageId)
  const quickReplyDraftId = useValue(compose$.quickReplyDraftId)
  const quickReplyDraftSaved = useValue(compose$.quickReplyDraftSaved)
  const sendingDraftIds = useValue(compose$.sendingDraftIds)
  const tabs = useValue(compose$.tabs)
  const activeTab = useValue(compose$.activeTab)
  const activeTabData = tabs.find((tab) => tab.id === activeTab) ?? null
  const activeDocumentTab = activeTabData?.kind === 'reader' || activeTabData?.kind === 'compose' ? activeTabData : null
  const conversationActiveTab = activeDocumentTab ? activeTab : ''
  const listedActiveThread = useValue(getActiveThread)
  const activeThread =
    activeTabData?.kind === 'thread' && activeTabData.threadId === selectedThreadId
      ? threadFromTab(activeTabData, listedActiveThread)
      : listedActiveThread

  const detailsOpen = !!activeThread && !activeDocumentTab && mediaOpen
  // Slides open and shut beside the conversation on a wide window (index.css,
  // .details-slide), like the board's conversation pane.
  const detailsPhase = usePresence(detailsOpen, PANE_ANIMATION_MS)

  const activeAccount = activeThread
    ? accounts.find((account) => account.id === activeThread.account_id)
    : (accounts.find((account) => account.id === selectedAccountId) ?? null)
  const isRSS = isRssAccount(activeAccount ?? undefined, activeThread?.account_id ?? selectedAccountId)
  const activeThreadId = activeThread?.thread_id ?? ''
  const conversationWallpaper = wallpaperCss(activeAccount?.chat_wallpaper)
  // Show a spinner instead of the previous thread's messages while a freshly
  // selected thread loads. Suppressed once any loaded message belongs to the
  // active thread, so re-reads of the open thread (sync refresh) don't flash.
  const showThreadLoading = threadLoading && !messages.some((message) => message.thread_id === activeThreadId)
  // The load failed and there's nothing of this thread to show — offer a retry
  // instead of a blank pane. Suppressed while a (re)load is in flight.
  const showThreadError =
    !showThreadLoading &&
    threadErrorId === activeThreadId &&
    !messages.some((message) => message.thread_id === activeThreadId)
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
  const composerRef = useRef<HTMLDivElement | null>(null)
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
    if (!host || !composer) return
    host.style.setProperty(COMPOSER_OVERLAP_VAR, `${composer.offsetHeight}px`)
    return () => {
      host.style.removeProperty(COMPOSER_OVERLAP_VAR)
    }
  }, [hasConversation, isRSS])
  const conversationScroll = useConversationScroll(activeThreadId, displayMessages, conversationActiveTab, unreadKey)
  const { scrollRef, setScrollTop } = conversationScroll

  const contextMenu = useValue(thread$.messageContextMenu)

  // Files dropped anywhere on the open conversation go to the reply as
  // attachments; feeds have no reply box, and a reader tab in front isn't the
  // conversation. Files that arrive late (Linux) are for the conversation they
  // were dropped on, not the one open by then. A drag that may be a link
  // (Linux) must still reach the reply box, so the overlay is only drawn; the
  // message bodies' frames, which would swallow the drop, then let it through
  // to the pane.
  const {
    dragging: fileDragging,
    mayBeLink,
    dropHandlers: { ref: dropRef, ...dropHandlers },
  } = useFileDrop(addQuickReplyAttachments, {
    enabled: !!activeThread && !isRSS && !activeDocumentTab,
    scope: activeThreadId,
  })
  // The conversation element is both the drop area and `conversationRef`.
  const setConversationElement = useCallback(
    (element: HTMLDivElement | null) => {
      conversationRef.current = element
      dropRef(element)
    },
    [dropRef],
  )

  const showMessageInConversation = useCallback((messageId: string) => {
    thread$.pendingScrollMessageId.set(messageId)
    thread$.galleryIndex.set(null)
    thread$.mediaOpen.set(false)
  }, [])

  useEffect(() => {
    resetThreadView()
  }, [activeThreadId])

  // Keeps the overlap in step as the composer grows with a longer reply or an
  // attachment, and a reader at the bottom parked there. That goes through the
  // scroll controller like any other positioning of ours, or the scroll it
  // causes would read as the reader's and drop a settling pin.
  useLayoutEffect(() => {
    const host = conversationRef.current
    const container = scrollRef.current
    const composer = composerRef.current
    if (!host || !container || !composer) return
    const observer = new ResizeObserver(() => {
      const atBottom = container.scrollHeight - container.scrollTop - container.clientHeight <= 1
      host.style.setProperty(COMPOSER_OVERLAP_VAR, `${composer.offsetHeight}px`)
      if (atBottom) setScrollTop(container.scrollHeight)
    })
    observer.observe(composer)
    return () => observer.disconnect()
  }, [hasConversation, isRSS, scrollRef, setScrollTop])

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
    if (!threadSearchOpen) return
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
  }, [threadSearchOpen, threadSearchFocus])

  // Bring the message holding the active match into view — which also mounts an
  // HTML body's frame. Each body then scrolls to the exact <mark> it rendered,
  // including when the search moves between two matches in the same message,
  // which is why the offset is deliberately not a dependency here.
  useEffect(() => {
    if (!activeSearchId) return
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

      {!isRSS && (
        <div
          ref={composerRef}
          className={`pointer-events-none absolute inset-x-0 bottom-0 z-10 ${
            conversationWallpaper.className === 'wallpaper-preset-plain' ? '[--me-composer-halo:var(--me-bg-chat)]' : ''
          }`}
        >
          <QuickReplyComposer />
        </div>
      )}
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
    <div
      className={`relative flex flex-1 flex-col overflow-hidden ${
        mobilePane === 'conversation' ? 'max-[768px]:flex' : 'max-[768px]:hidden'
      }`}
    >
      <ConversationTabs />
      {activeThread && !activeDocumentTab && (
        <ConversationHeader activeThread={activeThread} desktopSearchInputRef={desktopThreadSearchInputRef} />
      )}
      <div className="relative flex min-h-0 flex-1 overflow-hidden">
        <section className="relative flex flex-1 flex-col overflow-hidden bg-chat">
          <div
            ref={setConversationElement}
            aria-hidden={!!activeDocumentTab}
            className={`relative flex min-h-0 flex-1 flex-col ${
              activeDocumentTab ? 'pointer-events-none invisible' : ''
            } ${fileDragging && mayBeLink ? '[&_iframe]:pointer-events-none' : ''}`}
            {...dropHandlers}
          >
            {conversationContent}
            {fileDragging && <DropOverlay passThrough={mayBeLink} />}
          </div>
          {activeDocumentTab && (
            <div className="absolute inset-0 z-20 flex flex-col bg-chat">
              <ReaderTabView tab={activeDocumentTab} />
            </div>
          )}
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

      {galleryIndex !== null && galleryItems[galleryIndex] && (
        <Gallery
          items={galleryItems}
          index={galleryIndex}
          onIndexChange={(idx) => thread$.galleryIndex.set(idx)}
          onClose={() => thread$.galleryIndex.set(null)}
        />
      )}

      {contextMenu && (
        <MessageContextMenu state={contextMenu} isRSS={isRSS} onClose={() => thread$.messageContextMenu.set(null)} />
      )}
    </div>
  )
}
