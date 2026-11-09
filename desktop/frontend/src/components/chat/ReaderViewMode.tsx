import { Code, FileText } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'

/** Shared HTML/plain controls for the tab and standalone message readers. */
export function ReaderViewMode({
  mode,
  onChange,
}: {
  mode: 'html' | 'plain'
  onChange: (mode: 'html' | 'plain') => void
}) {
  const { t } = useTranslation()
  return (
    <div className="flex items-center gap-0.5 rounded-full bg-hover p-0.5">
      {(['html', 'plain'] as const).map((value) => (
        <button
          key={value}
          type="button"
          onClick={() => onChange(value)}
          aria-pressed={mode === value}
          className={`flex h-7 items-center gap-1 rounded-full px-2.5 text-[0.6875rem] font-semibold cursor-pointer ${
            mode === value ? 'bg-chats text-accent shadow-sm dark:shadow-black/10' : 'text-secondary hover:text-primary'
          }`}
          title={t(value === 'html' ? 'chat.htmlView' : 'chat.plainView')}
        >
          {value === 'html' ? <Code size={13} /> : <FileText size={13} />}
          {value === 'html' ? 'HTML' : t('settings.account.conversationPlain')}
        </button>
      ))}
    </div>
  )
}
