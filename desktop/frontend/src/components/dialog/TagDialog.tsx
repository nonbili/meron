import { useEffect, useState } from 'react'
import { Tag, X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { loadTagCounts, setThreadTags, threadTags, type TagCount } from '../../states/mailTags'
import { ui$ } from '../../states/ui'
import { IconButton } from '../button/IconButton'
import { Dialog } from './Dialog'

export function TagDialog({ threadId }: { threadId: string }) {
  const { t } = useTranslation()
  const [tags, setTags] = useState(() => threadTags(threadId))
  const [known, setKnown] = useState<TagCount[]>([])
  const [input, setInput] = useState('')
  const [invalid, setInvalid] = useState(false)

  useEffect(() => {
    void loadTagCounts().then(setKnown, () => setKnown([]))
  }, [])

  const onClose = () => ui$.tagThreadId.set('')

  const change = async (add: string[], remove: string[]) => {
    try {
      setTags(await setThreadTags(threadId, add, remove))
      setInput('')
      setInvalid(false)
    } catch {
      setInvalid(true)
    }
  }

  const query = input.trim().toLowerCase()
  const suggestions = known
    .filter(({ tag }) => !tags.includes(tag) && tag.includes(query))
    .sort((a, b) => b.count - a.count)
    .slice(0, 12)

  return (
    <Dialog onClose={onClose} aria-label={t('tags.title')} className="max-w-sm p-6 flex flex-col gap-4">
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-3 min-w-0">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent">
            <Tag size={17} />
          </div>
          <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight">{t('tags.title')}</h2>
        </div>
        <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
      </div>

      {tags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {tags.map((tag) => (
            <button
              key={tag}
              onClick={() => void change([], [tag])}
              aria-label={t('tags.remove', { tag })}
              className="flex items-center gap-1 rounded-lg bg-accent/10 px-2 py-1 text-xs font-medium text-accent cursor-pointer hover:bg-accent/20"
            >
              {tag} <X size={11} />
            </button>
          ))}
        </div>
      )}

      <div className="flex flex-col gap-2">
        <input
          autoFocus
          value={input}
          onChange={(event) => {
            setInput(event.target.value)
            setInvalid(false)
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && input.trim()) void change([input], [])
          }}
          placeholder={t('tags.placeholder')}
          className="w-full rounded-xl bg-hover px-3.5 py-2.5 text-[0.8125rem] text-primary placeholder-secondary focus:ring-1 focus:ring-accent focus:bg-chats border border-transparent transition-all duration-150"
        />
        {invalid && <p className="text-[0.6875rem] text-rose-500 px-1 font-medium">{t('tags.invalid')}</p>}
      </div>

      {suggestions.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {suggestions.map(({ tag }) => (
            <button
              key={tag}
              onClick={() => void change([tag], [])}
              className="rounded-lg bg-hover px-2 py-1 text-xs font-medium text-secondary cursor-pointer hover:text-primary"
            >
              {tag}
            </button>
          ))}
        </div>
      )}
    </Dialog>
  )
}
