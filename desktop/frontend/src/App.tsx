import { useValue } from '@legendapp/state/react'
import { useTranslation } from './lib/i18n'
import { ui$ } from './states/ui'
import { mail$ } from './states/mail'
import { compose$ } from './states/composeState'
import { accounts$ } from './states/accounts'
import { kanban$ } from './states/kanban'
import { settings$ } from './states/settings'
import { startKanbanResize, startThreadListResize } from './lib/paneResize'
import { useAppEffects } from './useAppEffects'
import { SideNav } from './components/sidenav/SideNav'
import { ThreadList } from './components/threads/ThreadList'
import { KanbanView } from './components/kanban/KanbanView'
import { KanbanConversationPane } from './components/kanban/KanbanConversationPane'
import { TasksPanel } from './components/tasks/TasksPanel'
import { TasksSlide } from './components/tasks/TasksSlide'
import { MessagePane } from './components/chat/MessagePane'
import { AboutDialog } from './components/dialog/AboutDialog'
import { ChangelogDialog } from './components/dialog/ChangelogDialog'
import { CommandPalette } from './components/palette/CommandPalette'
import { ErrorBoundary } from './components/ErrorBoundary'
import { AppHotkeys } from './components/dialog/AppHotkeys'
import { QuitHotkey } from './components/dialog/QuitHotkey'
import { ShortcutsDialog } from './components/dialog/ShortcutsDialog'
import { AppToast } from './components/toast/AppToast'
import { EmlExportProgress } from './components/toast/EmlExportProgress'
import { McpApprovalDialog } from './components/dialog/McpApprovalDialog'
import { AppConfirm } from './components/dialog/AppConfirm'
import { CertificateTrustDialog } from './components/dialog/CertificateTrustDialog'
import { TitleBar, useTitleBar } from './components/titlebar/TitleBar'
import { ConnectivityBanner } from './components/banner/ConnectivityBanner'
import { UpdateBanner } from './components/banner/UpdateBanner'
import { SetupScreen } from './components/setup/SetupScreen'
import { AccountDialog } from './components/dialog/AccountDialog'
import { SettingsDialog } from './components/dialog/SettingsDialog'
import { AddFeedDialog } from './components/dialog/AddFeedDialog'
import { FeedEditDialog } from './components/dialog/FeedEditDialog'

export default function App() {
  const { t } = useTranslation()
  const system = useValue(ui$.system)
  const accounts = useValue(accounts$)
  const activeBoardId = useValue(kanban$.activeBoardId)
  const tasksPanelOpen = useValue(ui$.tasksPanelOpen)
  const activeTaskList = useValue(ui$.activeTaskList)
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
  // The side navigation (and Meron's title bar, when there is one) form a
  // frame around the content, which sits on its own rounded surface (Slack,
  // Discord, Teams). Under the system title bar the surface also stands clear
  // of the top edge, so it looks the same in both modes.
  const titleBar = useTitleBar()

  const showKanbanMessagePane = !!kanbanPaneThreadId || composeTabs.some((tab) => tab.id === activeComposeTab)

  if (system && accounts.length === 0) {
    return (
      <div className="flex h-full w-full flex-col bg-app text-primary">
        <TitleBar tools={false} />
        <div className="min-h-0 flex-1">
          <SetupScreen />
        </div>
        <QuitHotkey />
      </div>
    )
  }

  return (
    <div className="flex h-full w-full flex-col bg-app text-primary">
      <TitleBar />
      <ConnectivityBanner />
      <UpdateBanner />
      <main className="flex min-h-0 w-full flex-1 bg-sidenav">
        <ErrorBoundary label="side navigation">
          <SideNav />
        </ErrorBoundary>
        <div className={titleBar ? 'app-frame-content' : 'app-frame-content app-frame-detached'}>
          <ErrorBoundary label="thread list">
            {activeBoardId ? (
              <KanbanView
                boardId={activeBoardId}
                aside={
                  <KanbanConversationPane
                    open={showKanbanMessagePane}
                    widthPercent={kanbanPaneWidth}
                    resizeTitle={t('layout.resizeConversation')}
                    onResizeStart={startKanbanResize}
                  >
                    <ErrorBoundary label="conversation">
                      <MessagePane />
                    </ErrorBoundary>
                  </KanbanConversationPane>
                }
              />
            ) : (
              <ThreadList width={threadListWidth} onResizeStart={startThreadListResize} />
            )}
          </ErrorBoundary>
          {/* On a board the conversation opens inside KanbanView, under its header. */}
          {!activeBoardId && (
            <ErrorBoundary label="conversation">
              <MessagePane />
            </ErrorBoundary>
          )}

          {/* Tasks is a panel, not a view: it sits to the right of whatever is
          open so a list can be worked against the thread list beside it. */}
          <TasksSlide open={tasksPanelOpen && !!activeTaskList}>
            {activeTaskList ? (
              <ErrorBoundary label="tasks">
                <TasksPanel listId={activeTaskList} />
              </ErrorBoundary>
            ) : null}
          </TasksSlide>
        </div>

        <AppHotkeys />
        <QuitHotkey />
        <CommandPalette />
        <ShortcutsDialog />
        <AboutDialog />
        <ChangelogDialog />

        {/* Settings first so the add-account dialog (setupOpen), opened from within
          Settings, layers on top and returns here when closed. */}
        {settingsOpen && <SettingsDialog />}
        {setupOpen && <AccountDialog />}
        {addFeedAccount && <AddFeedDialog />}
        {editFeed && <FeedEditDialog />}

        <AppToast />
        <EmlExportProgress />
        <AppConfirm />
        <McpApprovalDialog />
        <CertificateTrustDialog />
      </main>
    </div>
  )
}
