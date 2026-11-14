import { useState } from 'react'
import { X, Rss, Trash2, Copy, Check } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { Dialog } from './Dialog'
import { copyText } from '../../lib/native'
import { removeFeed, renameFeed } from '../../states/feeds'
import { ui$ } from '../../states/ui'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { TextInput } from '../field/Field'

export function FeedEditDialog() {
  const { t } = useTranslation()
  const feed = useValue(ui$.editFeed)
  const [confirming, setConfirming] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [copied, setCopied] = useState(false)
  // The dialog is mounted per feed, so the feed's listed name seeds the field.
  const listed = feed?.title ?? feed?.name ?? ''
  const [name, setName] = useState(listed)
  const [saving, setSaving] = useState(false)

  const onClose = () => {
    if (deleting || saving) return
    ui$.editFeed.set(null)
  }

  if (!feed) return null

  const onCopy = async () => {
    if (!feed.url) return
    await copyText(feed.url)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  const renamed = name.trim() !== listed.trim()
  const onRename = async () => {
    if (saving || !renamed) return
    setSaving(true)
    await renameFeed(feed.threadId, name.trim())
    setSaving(false)
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
    <Dialog
      onClose={onClose}
      dismissible={!deleting && !saving}
      closeOnBackdrop={false}
      aria-label={feed.name}
      className="max-w-md p-6 flex flex-col gap-5"
    >
      {/* Header */}
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3 min-w-0">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent">
            <Rss size={17} />
          </div>
          <div className="min-w-0">
            <h2 className="text-[0.9375rem] font-bold tracking-tight leading-tight truncate">{feed.name}</h2>
            <p className="text-[0.65625rem] text-secondary mt-1 font-medium truncate">
              {feed.url || t('feeds.manageSubscription')}
            </p>
          </div>
        </div>
        <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
      </div>

      {/* Name in the list */}
      <div className="flex flex-col gap-2">
        <label className="text-[0.6875rem] font-semibold text-secondary px-1">{t('feeds.name')}</label>
        <div className="flex items-center gap-2">
          <TextInput
            fieldSize="lg"
            surface="hover"
            value={name}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              // Enter that confirms an IME conversion is not a submit.
              if (event.nativeEvent.isComposing || event.keyCode === 229) return
              if (event.key === 'Enter') void onRename()
            }}
            disabled={saving}
            className="flex-1"
          />
          <Button onClick={onRename} disabled={saving || !renamed}>
            {t('buttons.save')}
          </Button>
        </div>
        <p className="text-[0.65625rem] text-secondary px-1 leading-relaxed font-medium">{t('feeds.nameHint')}</p>
      </div>

      {/* Feed URL */}
      {feed.url && (
        <div className="flex flex-col gap-2">
          <label className="text-[0.6875rem] font-semibold text-secondary px-1">{t('feeds.url')}</label>
          <div className="flex items-center gap-2 rounded-xl bg-hover px-3 py-2">
            <span className="flex-1 min-w-0 truncate text-[0.6875rem] font-medium text-primary">{feed.url}</span>
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
        <label className="text-[0.6875rem] font-semibold text-secondary px-1">{t('feeds.actions.deleteFeed')}</label>
        <p className="text-[0.65625rem] text-secondary px-1 leading-relaxed font-medium">{t('feeds.deleteHint')}</p>
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
    </Dialog>
  )
}
