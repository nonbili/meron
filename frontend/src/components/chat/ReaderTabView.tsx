import { Code, FileText, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { closeMessageTab, setTabViewMode } from '../../states/compose'
import type { MessageTab } from '../../types'
import { Composer } from '../composer/Composer'
import { HtmlMessageView } from './HtmlMessageView'
import { AddressRow } from './AddressList'
import { formatFullTimestamp } from './messageHelpers'

// Renders the active reader/compose tab: a compose tab shows the Composer, a
// reader tab shows the message header, address rows and the HTML/plain body.
export function ReaderTabView({ tab }: { tab: MessageTab }) {
  const { t } = useTranslation()
  if (tab.kind === 'compose') {
    return <Composer key={tab.id} tabId={tab.id} />
  }

  const hasAddresses = tab.fromRaw || tab.to || tab.cc || tab.bcc || tab.replyTo

  return (
    <>
      <header className="flex h-16 shrink-0 items-center gap-3 border-b border-border bg-header px-4 z-10 select-none">
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-sm font-bold text-primary leading-tight" title={tab.subject}>
            {tab.subject}
          </h2>
          {tab.date && (
            <p className="truncate text-[10.5px] text-secondary mt-0.5 font-medium">{formatFullTimestamp(tab.date)}</p>
          )}
        </div>
        <div className="flex items-center gap-1 rounded-lg bg-hover p-0.5">
          <button
            onClick={() => setTabViewMode(tab.id, 'html')}
            className={`flex items-center gap-1 rounded-md px-2.5 py-1 text-[11px] font-semibold cursor-pointer ${
              tab.viewMode === 'html' ? 'bg-chats text-accent shadow-sm' : 'text-secondary hover:text-primary'
            }`}
            title={t('chat.htmlView')}
          >
            <Code size={13} /> HTML
          </button>
          <button
            onClick={() => setTabViewMode(tab.id, 'plain')}
            className={`flex items-center gap-1 rounded-md px-2.5 py-1 text-[11px] font-semibold cursor-pointer ${
              tab.viewMode === 'plain' ? 'bg-chats text-accent shadow-sm' : 'text-secondary hover:text-primary'
            }`}
            title={t('chat.plainView')}
          >
            <FileText size={13} /> {t('settings.account.conversationPlain')}
          </button>
        </div>
        <button
          onClick={() => closeMessageTab(tab.id)}
          className="flex h-9 w-9 items-center justify-center rounded-full hover:bg-hover text-secondary cursor-pointer"
          title={t('chat.closeTab')}
        >
          <X size={16} />
        </button>
      </header>
      {hasAddresses && (
        <div className="shrink-0 space-y-1 border-b border-border bg-header px-4 py-2.5 text-secondary select-text">
          {tab.fromRaw && <AddressRow label={t('composer.fields.from')} rawList={tab.fromRaw} />}
          {tab.to && <AddressRow label={t('composer.fields.to')} rawList={tab.to} />}
          {tab.cc && <AddressRow label={t('composer.fields.cc')} rawList={tab.cc} />}
          {tab.bcc && <AddressRow label={t('composer.fields.bcc')} rawList={tab.bcc} />}
          {tab.replyTo && <AddressRow label="Reply-To" rawList={tab.replyTo} />}
        </div>
      )}
      <HtmlMessageView
        scrollKey={tab.id}
        title={tab.subject}
        html={tab.bodyHtml}
        text={tab.body}
        viewMode={tab.viewMode}
      />
    </>
  )
}
