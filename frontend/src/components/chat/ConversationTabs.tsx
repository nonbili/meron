import { MessageSquare, SquarePen, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from 'react-i18next'
import { compose$, closeMessageTab } from '../../states/compose'
import { ui$ } from '../../states/ui'

// The tab strip above the conversation: a fixed "Conversation" tab plus open
// thread, reader and compose tabs. Renders nothing when no tabs are open.
export function ConversationTabs() {
  const { t } = useTranslation()
  const tabs = useValue(compose$.tabs)
  const activeTab = useValue(compose$.activeTab)
  if (tabs.length === 0) return null

  return (
    <div className="flex h-10 shrink-0 items-stretch gap-1 overflow-x-auto border-b border-border bg-header px-2 select-none">
      <button
        onClick={() => compose$.activeTab.set('')}
        className={`flex items-center gap-1.5 px-3 text-xs font-semibold border-b-2 transition-colors cursor-pointer ${
          activeTab === '' ? 'border-accent text-accent' : 'border-transparent text-secondary hover:text-primary'
        }`}
        title={t('chat.currentConversation')}
      >
        <MessageSquare size={13} />
        {t('chat.current')}
      </button>
      {tabs.map((tab) => (
        <div
          key={tab.id}
          onClick={() => {
            if (tab.kind === 'thread') ui$.selectedThread.set(tab.threadId)
            compose$.activeTab.set(tab.id)
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
              closeMessageTab(tab.id)
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
