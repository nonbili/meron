import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { compose$, openComposeTab } from '../../states/compose'
import { ui$ } from '../../states/ui'
import { mail$, getActiveThread, loadThread } from '../../states/mail'
import { accounts$ } from '../../states/accounts'
import { resetThreadView, thread$, type ConversationMode } from '../../states/thread'
import { Gallery } from './Gallery'
import { ConversationDetailsPanel } from './ConversationDetailsPanel'
import { EmptyState } from '../empty-state/EmptyState'
import { QuickReplyComposer } from './QuickReplyComposer'
import { ConversationTabs } from './ConversationTabs'
import { ReaderTabView } from './ReaderTabView'
import { ConversationHeader } from './ConversationHeader'
import { ThreadSearchBarMobile } from './ThreadSearchBarMobile'
import { ConversationMessageList } from './ConversationMessageList'
import { MessageContextMenu, type MessageContextMenuState } from './MessageContextMenu'
import { buildGalleryItems, buildThreadMedia, buildParticipants } from './conversationMedia'
import { useThreadSearch } from './useThreadSearch'
import { useConversationScroll } from './useConversationScroll'
import { ArrowRight } from 'lucide-react'
import { wallpaperCss } from '../../lib/wallpapers'
import { clearMediaSession } from '../../lib/mediaSession'
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
  const galleryIndex = useValue(thread$.galleryIndex)
  const mediaOpen = useValue(thread$.mediaOpen)
  const modeOverrides = useValue(thread$.conversationModeOverrides)
  // Ring-highlight for a message jumped to from the starred list; shares the
  // search-match ring styling in ConversationMessageList.
  const flashMessageId = useValue(thread$.flashMessageId)
  const quickReplyDraftId = useValue(compose$.quickReplyDraftId)
  const quickReplyDraftSaved = useValue(compose$.quickReplyDraftSaved)
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

  const activeAccount = activeThread
    ? accounts.find((account) => account.id === activeThread.account_id)
    : (accounts.find((account) => account.id === selectedAccountId) ?? null)
  const isRSS = activeAccount?.provider === 'rss' || activeAccount?.auth_type === 'rss'
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
  const unreadKey = messages.map((message) => `${message.id}:${message.unread ? '1' : '0'}`).join('|')
  // Hide the tail draft message from the rendered conversation once quick
  // reply has hydrated its content for inline editing — otherwise the same
  // draft text would appear twice (as a bubble and pre-filled in the reply
  // bar). Older draft messages elsewhere in the thread's history stay visible.
  const displayMessages = useMemo(() => {
    if (!quickReplyDraftSaved || !quickReplyDraftId) return messages
    const tail = messages[messages.length - 1]
    if (!tail) return messages
    const tailId = tail.message_id || tail.id
    return tailId === quickReplyDraftId ? messages.slice(0, -1) : messages
  }, [messages, quickReplyDraftId, quickReplyDraftSaved])

  const accountConversationMode: ConversationMode = (activeAccount?.conversation_html ?? true) ? 'html' : 'plain'
  const conversationMode: ConversationMode = activeAccount
    ? (modeOverrides[activeAccount.id] ?? accountConversationMode)
    : 'plain'
  const setQuickConversationMode = useCallback(
    (mode: ConversationMode) => {
      if (!activeAccount) return
      thread$.conversationModeOverrides[activeAccount.id].set(mode)
    },
    [activeAccount],
  )

  const { galleryItems, galleryOffsets } = useMemo(
    () => buildGalleryItems(messages, accounts, revealedRemote),
    [messages, accounts, revealedRemote],
  )
  const { mediaItems, fileItems } = useMemo(
    () => buildThreadMedia(messages, accounts, revealedRemote),
    [messages, accounts, revealedRemote],
  )
  const participants = useMemo(() => buildParticipants(messages, accounts, isRSS), [messages, accounts, isRSS])

  const desktopThreadSearchInputRef = useRef<HTMLInputElement | null>(null)
  const mobileThreadSearchInputRef = useRef<HTMLInputElement | null>(null)
  const { threadSearchOpen, searchMatches, activeSearchIndex, activeSearchId, goToSearchMatch } =
    useThreadSearch(messages)
  const { scrollRef, bottomAnchorRef, messagesWrapperRef, handleConversationScroll } = useConversationScroll(
    activeThreadId,
    messages,
    conversationActiveTab,
    unreadKey,
  )

  const [contextMenu, setContextMenu] = useState<MessageContextMenuState | null>(null)

  useEffect(() => {
    resetThreadView()
  }, [activeThreadId])

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
    const frame = window.requestAnimationFrame(() => {
      const input = window.matchMedia('(min-width: 900px)').matches
        ? desktopThreadSearchInputRef.current
        : mobileThreadSearchInputRef.current
      input?.focus()
    })
    return () => window.cancelAnimationFrame(frame)
  }, [threadSearchOpen])

  useEffect(() => {
    if (!activeSearchId) return
    const container = scrollRef.current
    const match = container?.querySelector<HTMLElement>(`[data-search-match-id="${CSS.escape(activeSearchId)}"]`)
    match?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }, [activeSearchId])

  const conversationContent = activeThread ? (
    <>
      <ConversationHeader
        activeThread={activeThread}
        isRSS={isRSS}
        conversationMode={conversationMode}
        setQuickConversationMode={setQuickConversationMode}
        searchMatches={searchMatches}
        activeSearchIndex={activeSearchIndex}
        goToSearchMatch={goToSearchMatch}
        desktopSearchInputRef={desktopThreadSearchInputRef}
      />

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

      {threadSearchOpen && (
        <ThreadSearchBarMobile
          searchMatches={searchMatches}
          activeSearchIndex={activeSearchIndex}
          goToSearchMatch={goToSearchMatch}
          inputRef={mobileThreadSearchInputRef}
        />
      )}

      <ConversationMessageList
        messages={displayMessages}
        showThreadLoading={showThreadLoading}
        showThreadError={showThreadError}
        onRetryThreadLoad={retryThreadLoad}
        messagesCursor={messagesCursor}
        messagesLoadingMore={messagesLoadingMore}
        activeThreadId={activeThreadId}
        searchMatches={searchMatches}
        activeSearchId={activeSearchId || flashMessageId}
        galleryOffsets={galleryOffsets}
        scrollRef={scrollRef}
        messagesWrapperRef={messagesWrapperRef}
        bottomAnchorRef={bottomAnchorRef}
        wallpaperClassName={conversationWallpaper.className}
        wallpaperStyle={conversationWallpaper.style}
        onScroll={handleConversationScroll}
        onOpenContextMenu={setContextMenu}
      />

      {!isRSS && <QuickReplyComposer />}
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
      <div className="relative flex min-h-0 flex-1 overflow-hidden">
        <section className="relative flex flex-1 flex-col overflow-hidden bg-chat">
          <div
            aria-hidden={!!activeDocumentTab}
            className={`relative flex min-h-0 flex-1 flex-col ${
              activeDocumentTab ? 'pointer-events-none invisible' : ''
            }`}
          >
            {conversationContent}
          </div>
          {activeDocumentTab && (
            <div className="absolute inset-0 z-20 flex flex-col bg-chat">
              <ReaderTabView tab={activeDocumentTab} />
            </div>
          )}
        </section>

        {activeThread && !activeDocumentTab && mediaOpen && (
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
            onComposeTo={(person) =>
              openComposeTab({
                accountId: activeThread.account_id,
                to: person.name && person.name !== person.email ? `${person.name} <${person.email}>` : person.email,
              })
            }
            onClose={() => thread$.mediaOpen.set(false)}
          />
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

      {contextMenu && <MessageContextMenu state={contextMenu} isRSS={isRSS} onClose={() => setContextMenu(null)} />}
    </div>
  )
}
