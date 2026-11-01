import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { Pencil, Tag, Trash2, X } from 'lucide-react'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { useTranslation } from '../../lib/i18n'
import { loadTagCounts, replaceTag, type TagCount } from '../../states/mailTags'
import { confirmAction } from '../../states/ui'
import { IconButton } from '../button/IconButton'
import { TextInput } from '../field/Field'
import { SettingRow, SettingsGroup } from './AccountSettingsRows'
import { Dialog } from './Dialog'

const nameClass =
  'min-w-0 flex-1 rounded-lg bg-hover px-2 py-1 text-xs text-primary outline-none ring-1 ring-border focus:ring-accent'

// A row that opens a dialog, so a long list of tags never pushes the settings
// below it out of sight.
export function TagsSection() {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  return (
    <SettingsGroup title={t('tags.title')}>
      <SettingRow
        title={t('tags.title')}
        hint={t('tags.manage.hint')}
        control={
          <button
            onClick={() => setOpen(true)}
            aria-label={t('tags.title')}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
          >
            {t('buttons.edit')}
          </button>
        }
      />
      {open && (
        <TagSettingsDialog title={t('tags.title')} hint={t('tags.manage.hint')} onClose={() => setOpen(false)}>
          <TagList />
        </TagSettingsDialog>
      )}
    </SettingsGroup>
  )
}

function TagSettingsDialog({
  title,
  hint,
  onClose,
  children,
}: {
  title: string
  hint: string
  onClose: () => void
  children: ReactNode
}) {
  const { t } = useTranslation()
  return (
    <Dialog
      onClose={onClose}
      closeOnBackdrop={false}
      aria-label={title}
      backdropClassName="z-[70] bg-black/45 dark:bg-black/65"
      className="flex max-w-md h-[520px] max-h-[85vh] flex-col overflow-hidden"
    >
      <div className="flex shrink-0 items-center justify-between gap-3 border-b border-border/70 px-6 py-4">
        <div className="flex items-center gap-2">
          <Tag className="text-accent" size={16} />
          <h3 className="text-[0.875rem] font-bold leading-tight">{title}</h3>
        </div>
        <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
      </div>
      <div className="flex flex-1 flex-col gap-3 overflow-hidden px-6 py-4">
        <p className="text-[0.75rem] text-secondary">{hint}</p>
        {children}
      </div>
    </Dialog>
  )
}

// Every tag in use, each to rename or delete across all mail on this device.
export function TagList() {
  const { t } = useTranslation()
  const [tags, setTags] = useState<TagCount[]>([])
  const [editing, setEditing] = useState<string | null>(null)
  const [name, setName] = useState('')
  const [invalid, setInvalid] = useState(false)
  const [busy, setBusy] = useState(false)
  const [filter, setFilter] = useState('')

  // On top of the settings dialog's own handler: Escape leaves the name, not settings.
  useEscapeKey(() => setEditing(null), editing !== null)

  const reload = () => loadTagCounts().then(setTags, () => setTags([]))

  useEffect(() => {
    void reload()
  }, [])

  const replace = async (tag: string, to?: string) => {
    setBusy(true)
    try {
      await replaceTag(tag, to)
      await reload()
      setEditing(null)
    } catch {
      setInvalid(true)
    } finally {
      setBusy(false)
    }
  }

  const remove = async ({ tag, count }: TagCount) => {
    const confirmed = await confirmAction({
      title: t('tags.manage.delete'),
      message: t('tags.manage.deleteConfirm', { tag, count }),
      confirmLabel: t('buttons.delete'),
      cancelLabel: t('buttons.cancel'),
      tone: 'danger',
    })
    if (confirmed) await replace(tag)
  }

  const query = filter.trim().toLowerCase()
  const sorted = tags
    .filter(({ tag }) => tag.includes(query))
    .sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag))

  return (
    <>
      {tags.length > 0 && (
        <TextInput
          type="search"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder={t('tags.manage.filter')}
          aria-label={t('tags.manage.filter')}
          className="w-full"
        />
      )}
      <div className="flex-1 overflow-y-auto rounded-xl border border-border divide-y divide-border/40">
        {sorted.map((entry) =>
          editing === entry.tag ? (
            <div key={entry.tag} className="px-3.5 py-1.5">
              <input
                autoFocus
                value={name}
                disabled={busy}
                aria-label={t('tags.manage.rename')}
                onChange={(event) => {
                  setName(event.target.value)
                  setInvalid(false)
                }}
                onBlur={() => {
                  if (!busy) setEditing(null)
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' && name.trim()) void replace(entry.tag, name)
                }}
                className={`w-full ${nameClass}`}
              />
            </div>
          ) : (
            <div key={entry.tag} className="flex items-center gap-2 px-3.5 py-1.5 text-xs">
              <span className="min-w-0 flex-1 truncate font-medium text-primary" title={entry.tag}>
                {entry.tag}
              </span>
              <span className="shrink-0 text-secondary">{entry.count}</span>
              <IconButton
                icon={Pencil}
                iconSize={13}
                label={t('tags.manage.rename')}
                size="sm"
                disabled={busy}
                onClick={() => {
                  setName(entry.tag)
                  setInvalid(false)
                  setEditing(entry.tag)
                }}
              />
              <IconButton
                icon={Trash2}
                iconSize={13}
                label={t('tags.manage.delete')}
                size="sm"
                disabled={busy}
                onClick={() => void remove(entry)}
              />
            </div>
          ),
        )}
      </div>
      {invalid && <p className="text-[0.6875rem] font-medium text-rose-500">{t('tags.invalid')}</p>}
    </>
  )
}
