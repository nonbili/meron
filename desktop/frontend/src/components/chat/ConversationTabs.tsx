import { MessageSquare, SquarePen, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { closeMessageTab, activateConversationTab } from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { kanban$ } from '../../states/kanban'
import { ui$ } from '../../states/ui'

// The tab strip above the conversation: the "Current" tab (when a conversation
// is open behind the tabs) plus open thread, reader and compose tabs. Renders
// nothing when no tabs are open.
export function ConversationTabs() {
  const { t } = useTranslation()
  const tabs = useValue(compose$.tabs)
  const activeTab = useValue(compose$.activeTab)
  const conversationThread = useValue(compose$.conversationThread)
  const activeBoardId = useValue(kanban$.activeBoardId)
  const paneThreadId = useValue(kanban$.paneThreadId)
  // The Current tab returns to the conversation the pane was showing before a
  // tab took over it. With no such conversation there is nothing to return to:
  // it would blank the pane, and in kanban view — where the pane only exists
  // while a card or a tab is open — close it outright. So it is only offered
  // once there is a conversation behind the tabs. Kanban owns its open
  // conversation through paneThreadId; elsewhere it's the remembered thread.
  const hasCurrentConversation = activeBoardId ? !!paneThreadId : !!conversationThread
  if (tabs.length === 0) return null

  return (
    <div className="flex h-10 shrink-0 items-stretch gap-1 overflow-x-auto border-b border-border bg-header px-2 select-none">
      {hasCurrentConversation && (
        <button
          onClick={() => activateConversationTab()}
          className={`flex items-center gap-1.5 px-3 text-xs font-semibold border-b-2 transition-colors cursor-pointer ${
            activeTab === '' ? 'border-accent text-accent' : 'border-transparent text-secondary hover:text-primary'
          }`}
          title={t('chat.currentConversation')}
        >
          <MessageSquare size={13} />
          {t('chat.current')}
        </button>
      )}
      {tabs.map((tab) => (
        <div
          key={tab.id}
          onClick={() => {
            // Activate the tab before retargeting selectedThread so the Current
            // tab's remembered thread (conversationThread) isn't overwritten.
            compose$.activeTab.set(tab.id)
            if (tab.kind === 'thread') ui$.selectedThread.set(tab.threadId)
          }}
          className={`group flex max-w-[200px] cursor-pointer items-center gap-1.5 px-3 text-xs font-semibold border-b-2 transition-colors ${
            activeTab === tab.id ? 'border-accent text-accent' : 'border-transparent text-secondary hover:text-primary'
          }`}
          title={tab.subject}
        >
          {tab.kind === 'thread' && <MessageSquare size={12} className="shrink-0" />}
          {tab.kind === 'compose' && <SquarePen size={12} className="shrink-0" />}
          <span className="truncate">{tab.subject}</span>
          <button
            onClick={(event) => {
              event.stopPropagation()
              void closeMessageTab(tab.id)
            }}
            className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full text-secondary hover:bg-active hover:text-primary"
            title={t('chat.closeTab')}
          >
            <X size={11} />
          </button>
        </div>
      ))}
    </div>
  )
}
