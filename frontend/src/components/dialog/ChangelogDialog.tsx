import { useEffect, useState } from 'react'
import { X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { ui$ } from '../../states/ui'
import { fetchChangelog, type ChangelogRelease } from '../../lib/changelog'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

function formatDate(iso: string): string {
  if (!iso) return ''
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  return date.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })
}

export function ChangelogDialog() {
  const { t } = useTranslation()
  const open = useValue(ui$.changelogOpen)
  const [releases, setReleases] = useState<ChangelogRelease[] | null>(null)
  const [error, setError] = useState(false)

  const onClose = () => ui$.changelogOpen.set(false)

  useEffect(() => {
    if (!open) return
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        onClose()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [open])

  useEffect(() => {
    if (!open) return
    let cancelled = false
    setReleases(null)
    setError(false)
    fetchChangelog()
      .then((items) => {
        if (!cancelled) setReleases(items)
      })
      .catch(() => {
        if (!cancelled) setError(true)
      })
    return () => {
      cancelled = true
    }
  }, [open])

  if (!open) return null

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/35 p-4 backdrop-blur-[3px] dark:bg-black/60"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose()
      }}
    >
      <div
        className="flex max-h-[80vh] w-full max-w-md flex-col overflow-hidden rounded-3xl border border-border bg-chats text-primary shadow-2xl shadow-black/20 animate-slide-up dark:shadow-black/45"
        role="dialog"
        aria-modal="true"
        aria-label={t('changelog.title')}
      >
        <div className="flex items-center justify-between border-b border-border/70 px-5 py-3.5">
          <h2 className="text-sm font-bold tracking-tight">{t('changelog.title')}</h2>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4">
          {releases === null && !error && (
            <p className="py-8 text-center text-sm text-secondary">{t('changelog.loading')}</p>
          )}
          {error && <p className="py-8 text-center text-sm text-secondary">{t('changelog.error')}</p>}
          {releases !== null && !error && releases.length === 0 && (
            <p className="py-8 text-center text-sm text-secondary">{t('changelog.empty')}</p>
          )}
          {releases !== null && !error && releases.length > 0 && (
            <ol className="flex flex-col gap-5">
              {releases.map((release) => (
                <li key={release.tag}>
                  <div className="flex items-baseline justify-between gap-3">
                    <h3 className="text-base font-bold tracking-tight">{release.version}</h3>
                    <span className="text-xs font-semibold text-secondary">{formatDate(release.date)}</span>
                  </div>
                  <ul className="mt-2 list-disc space-y-1 pl-5 text-sm leading-6 text-secondary">
                    {release.notes.map((note, index) => (
                      <li key={index}>{note}</li>
                    ))}
                  </ul>
                </li>
              ))}
            </ol>
          )}
        </div>

        <div className="flex justify-end border-t border-border/70 px-5 py-4">
          <Button variant="secondary" onClick={onClose}>
            {t('buttons.close')}
          </Button>
        </div>
      </div>
    </div>
  )
}
