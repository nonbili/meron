import { lazy, Suspense, useRef } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from 'react-i18next'
import { ui$ } from './states/ui'
import { mail$ } from './states/mail'
import { compose$ } from './states/compose'
import { accounts$ } from './states/accounts'
import { kanban$ } from './states/kanban'
import { settings$ } from './states/settings'
import { startKanbanResize, startThreadListResize } from './lib/paneResize'
import { useAppEffects } from './useAppEffects'
import { AccountSwitcher } from './components/sidebar/AccountSwitcher'
import { ThreadList } from './components/threads/ThreadList'
import { KanbanView } from './components/kanban/KanbanView'
import { MessagePane } from './components/chat/MessagePane'
import { AboutDialog } from './components/dialog/AboutDialog'
import { CommandPalette } from './components/palette/CommandPalette'
import { ErrorBoundary } from './components/ErrorBoundary'
import { AppHotkeys } from './components/dialog/AppHotkeys'
import { ShortcutsDialog } from './components/dialog/ShortcutsDialog'
import { AppToast } from './components/toast/AppToast'
import { AppConfirm } from './components/dialog/AppConfirm'
import { MacTitleBar } from './components/titlebar/MacTitleBar'
import { ConnectivityBanner } from './components/banner/ConnectivityBanner'

// State-gated screens, code-split so they stay out of the initial bundle and only
// load when first opened. Each is a named export, so unwrap it to a default for lazy().
const SetupScreen = lazy(() => import('./components/setup/SetupScreen').then((m) => ({ default: m.SetupScreen })))
const AccountDialog = lazy(() =>
  import('./components/dialog/AccountDialog').then((m) => ({ default: m.AccountDialog })),
)
const SettingsDialog = lazy(() =>
  import('./components/dialog/SettingsDialog').then((m) => ({ default: m.SettingsDialog })),
)
const AddFeedDialog = lazy(() =>
  import('./components/dialog/AddFeedDialog').then((m) => ({ default: m.AddFeedDialog })),
)
const FeedEditDialog = lazy(() =>
  import('./components/dialog/FeedEditDialog').then((m) => ({ default: m.FeedEditDialog })),
)

export default function App() {
  const { t } = useTranslation()
  const mainRef = useRef<HTMLElement | null>(null)
  const system = useValue(ui$.system)
  const accounts = useValue(accounts$)
  const activeBoardId = useValue(kanban$.activeBoardId)
  const composeTabs = useValue(compose$.tabs)
  const activeComposeTab = useValue(compose$.activeTab)
  const threadListWidth = useValue(settings$.threadListWidth)
  const kanbanPaneThreadId = useValue(kanban$.paneThreadId)
  const kanbanPaneWidth = useValue(settings$.kanbanPaneWidth)
  const setupOpen = useValue(ui$.setupOpen)
  const settingsOpen = useValue(ui$.settingsOpen)
  const addFeedAccount = useValue(ui$.addFeedAccount)
  const editFeed = useValue(ui$.editFeed)

  useAppEffects()

  const showKanbanMessagePane = !!kanbanPaneThreadId || composeTabs.some((tab) => tab.id === activeComposeTab)

  if (system && accounts.length === 0) {
    return (
      <div className="flex h-full w-full flex-col bg-app text-primary">
        <MacTitleBar />
        <div className="min-h-0 flex-1">
          <Suspense fallback={null}>
            <SetupScreen />
          </Suspense>
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-full w-full flex-col bg-app text-primary">
      <MacTitleBar />
      <ConnectivityBanner />
      <main ref={mainRef} className="flex min-h-0 w-full flex-1 overflow-hidden">
        <ErrorBoundary label="sidebar">
          <AccountSwitcher />
        </ErrorBoundary>
        <ErrorBoundary label="thread list">
          {activeBoardId ? (
            <KanbanView boardId={activeBoardId} />
          ) : (
            <ThreadList width={threadListWidth} onResizeStart={startThreadListResize} />
          )}
        </ErrorBoundary>
        {!activeBoardId ? (
          <ErrorBoundary label="conversation">
            <MessagePane />
          </ErrorBoundary>
        ) : showKanbanMessagePane ? (
          <div
            className="relative flex shrink-0 overflow-hidden border-l border-border bg-chat max-[768px]:w-full"
            style={{ width: `${kanbanPaneWidth}%`, minWidth: 320 }}
          >
            <div
              className="absolute left-0 top-0 z-20 h-full w-2 -translate-x-1 cursor-col-resize"
              onPointerDown={(event) => startKanbanResize(event, mainRef.current)}
              title={t('layout.resizeConversation')}
            >
              <div className="mx-auto h-full w-px bg-transparent hover:bg-accent" />
            </div>
            <ErrorBoundary label="conversation">
              <MessagePane />
            </ErrorBoundary>
          </div>
        ) : null}

        <AppHotkeys />
        <CommandPalette />
        <ShortcutsDialog />
        <AboutDialog />

        {/* Settings first so the add-account dialog (setupOpen), opened from within
          Settings, layers on top and returns here when closed. Lazy-loaded, so a
          single Suspense boundary covers them while their chunk fetches. */}
        <Suspense fallback={null}>
          {settingsOpen && <SettingsDialog />}
          {setupOpen && <AccountDialog />}
          {addFeedAccount && <AddFeedDialog />}
          {editFeed && <FeedEditDialog />}
        </Suspense>

        <AppToast />
        <AppConfirm />
      </main>
    </div>
  )
}
