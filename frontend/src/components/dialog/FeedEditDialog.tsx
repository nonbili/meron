import { useState } from 'react'
import { X, Rss, Trash2, Copy, Check } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { removeFeed } from '../../states/feeds'
import { ui$ } from '../../states/ui'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

export function FeedEditDialog() {
  const { t } = useTranslation()
  const feed = useValue(ui$.editFeed)
  const [confirming, setConfirming] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [copied, setCopied] = useState(false)

  if (!feed) return null

  const onCopy = async () => {
    if (!feed.url) return
    await navigator.clipboard.writeText(feed.url)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  const onClose = () => {
    if (deleting) return
    ui$.editFeed.set(null)
  }

  const onDelete = async () => {
    if (deleting) return
    if (!confirming) {
      setConfirming(true)
      return
    }
    setDeleting(true)
    await removeFeed(feed.threadId)
    setDeleting(false)
  }

  return (
    <div className="fixed inset-0 flex items-center justify-center bg-black/40 dark:bg-black/60 backdrop-blur-[3px] z-50 p-4 select-none animate-fade-in">
      <div className="bg-chats border border-border text-primary max-w-md w-full rounded-3xl p-6 shadow-2xl animate-slide-up flex flex-col gap-5">
        {/* Header */}
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-center gap-3 min-w-0">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent">
              <Rss size={17} />
            </div>
            <div className="min-w-0">
              <h2 className="text-[15px] font-bold tracking-tight leading-tight truncate">{feed.name}</h2>
              <p className="text-[10.5px] text-secondary mt-1 font-medium truncate">
                {feed.url || t('feeds.manageSubscription')}
              </p>
            </div>
          </div>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>

        {/* Feed URL */}
        {feed.url && (
          <div className="flex flex-col gap-2">
            <label className="text-[11px] font-semibold text-secondary px-1">{t('feeds.url')}</label>
            <div className="flex items-center gap-2 rounded-xl bg-hover px-3 py-2">
              <span className="flex-1 min-w-0 truncate text-[11px] font-medium text-primary">{feed.url}</span>
              <button
                onClick={onCopy}
                className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg hover:bg-active text-secondary transition-colors cursor-pointer"
                title={copied ? t('common.copied') : t('feeds.copyUrl')}
              >
                {copied ? <Check size={14} className="text-emerald-500" /> : <Copy size={14} />}
              </button>
            </div>
          </div>
        )}

        {/* Danger zone */}
        <div className="flex flex-col gap-2">
          <label className="text-[11px] font-semibold text-secondary px-1">{t('feeds.actions.deleteFeed')}</label>
          <p className="text-[10.5px] text-secondary px-1 leading-relaxed font-medium">{t('feeds.deleteHint')}</p>
        </div>

        {/* Footer */}
        <div className="flex justify-between gap-2 select-none">
          <button
            onClick={onDelete}
            disabled={deleting}
            className="px-4 py-2 rounded-xl text-xs font-bold transition-all flex items-center gap-1.5 bg-rose-500/10 text-rose-500 hover:bg-rose-500 hover:text-white cursor-pointer disabled:opacity-50"
          >
            <Trash2 size={13} />
            <span>{confirming ? t('feeds.actions.confirmDelete') : t('feeds.actions.deleteFeed')}</span>
          </button>
          <Button variant="ghost" onClick={onClose} disabled={deleting}>
            {t('buttons.close')}
          </Button>
        </div>
      </div>
    </div>
  )
}
