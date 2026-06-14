import { useState } from 'react'
import { X, Rss, RefreshCw } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from 'react-i18next'
import { showToast } from '../../states/ui'
import { submitFeed } from '../../states/feeds'
import { ui$ } from '../../states/ui'
import { accounts$ } from '../../states/accounts'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

export function AddFeedDialog() {
  const { t } = useTranslation()
  const accountId = useValue(ui$.addFeedAccount)
  const accounts = useValue(accounts$)
  const [url, setUrl] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const account = accounts.find((acc) => acc.id === accountId)
  const accountName = account?.display_name || account?.email || t('accounts.thisAccount')

  const onClose = () => {
    if (loading) return
    ui$.addFeedAccount.set('')
  }

  const submit = async () => {
    const trimmed = url.trim()
    if (!trimmed || loading) return
    setLoading(true)
    setError('')
    try {
      await submitFeed(accountId, trimmed)
      showToast(t('feeds.added'))
      ui$.addFeedAccount.set('')
    } catch (err) {
      setError(err instanceof Error ? err.message : t('feeds.addFailed'))
    } finally {
      setLoading(false)
    }
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
              <h2 className="text-[15px] font-bold tracking-tight leading-tight">{t('feeds.actions.addFeed')}</h2>
              <p className="text-[10.5px] text-secondary mt-1 font-medium truncate">
                {t('feeds.subscribeUnder', { account: accountName })}
              </p>
            </div>
          </div>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>

        {/* Content */}
        <div className="flex flex-col gap-2">
          <label className="text-[11px] font-semibold text-secondary px-1">
            {t('feeds.url')}
          </label>
          <input
            autoFocus
            value={url}
            onChange={(event) => setUrl(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') submit()
              if (event.key === 'Escape') onClose()
            }}
            placeholder="https://example.com/feed.xml"
            className="w-full rounded-xl bg-hover px-3.5 py-2.5 text-[13px] text-primary placeholder-secondary focus:ring-1 focus:ring-accent focus:bg-chats border border-transparent transition-all duration-150"
          />
          <p className="text-[10.5px] text-secondary px-1 leading-relaxed font-medium">{t('feeds.urlHint')}</p>
          {error && <p className="text-[11px] text-rose-500 px-1 font-medium">{error}</p>}
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 select-none">
          <Button variant="ghost" onClick={onClose} disabled={loading}>
            {t('buttons.cancel')}
          </Button>
          <Button variant="primary" onClick={submit} disabled={loading || !url.trim()}>
            {loading && <RefreshCw size={11} className="animate-spin" />}
            <span>{t('feeds.actions.addFeed')}</span>
          </Button>
        </div>
      </div>
    </div>
  )
}
