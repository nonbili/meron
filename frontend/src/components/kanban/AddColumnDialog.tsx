import { useEffect, useMemo, useState } from 'react'
import { Inbox, Star, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { Folder } from '../../types'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { Checkbox } from '../field/Checkbox'
import { AccountSection } from './AccountSection'
import { buildFolderTree, type AccountGroup } from './folderTree'

export type { AccountGroup } from './folderTree'

export function AddColumnDialog({
  groups,
  initialSelected,
  inboxOption,
  specialOptions,
  onClose,
  onApply,
  onCreateFolder,
}: {
  groups: AccountGroup[]
  initialSelected: string[]
  /** Optional toggle for the board's pinned inbox column, shown above the folders. */
  inboxOption?: { key: string; label: string }
  /** Optional non-folder columns shown above account folders. */
  specialOptions?: { key: string; label: string; icon?: 'inbox' | 'star' }[]
  onClose: () => void
  onApply: (selectedKeys: string[]) => void
  onCreateFolder?: (accountId: string, name: string) => Promise<Folder>
}) {
  const { t } = useTranslation()
  const trees = useMemo(() => groups.map((group) => ({ ...group, tree: buildFolderTree(group.folders) })), [groups])
  const [selected, setSelected] = useState<Set<string>>(() => new Set(initialSelected))

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  function toggle(keys: string[], next: boolean) {
    setSelected((prev) => {
      const updated = new Set(prev)
      for (const key of keys) {
        if (next) updated.add(key)
        else updated.delete(key)
      }
      return updated
    })
  }

  const hasFolders = groups.some((group) => group.folders.length > 0)
  const topOptions = specialOptions ?? (inboxOption ? [{ ...inboxOption, icon: 'inbox' as const }] : [])

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 p-4" onClick={onClose}>
      <div
        className="flex max-h-[80vh] w-full max-w-md flex-col overflow-hidden rounded-2xl border border-border bg-chats shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-sm font-bold text-primary">{t('kanban.actions.addColumns')}</h2>
          <IconButton icon={X} label={t('buttons.close')} size="sm" radius="lg" onClick={onClose} />
        </div>
        <p className="shrink-0 px-4 pt-3 text-[11px] font-medium text-secondary">{t('kanban.addColumnsHint')}</p>
        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          {topOptions.map((option) => {
            const Icon = option.icon === 'star' ? Star : Inbox
            return (
              <label
                key={option.key}
                className="mb-1 flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-hover"
              >
                <Checkbox
                  checked={selected.has(option.key)}
                  onChange={(event) => toggle([option.key], event.target.checked)}
                />
                <Icon size={14} className="shrink-0 text-secondary" />
                <span className="truncate text-xs font-semibold text-primary">{option.label}</span>
              </label>
            )
          })}
          {!hasFolders && !onCreateFolder ? (
            <div className="px-3 py-8 text-center text-xs font-medium text-secondary">{t('folders.noneAvailable')}</div>
          ) : (
            trees.map((group) => (
              <AccountSection
                key={group.accountId}
                group={group}
                selected={selected}
                onToggle={toggle}
                onCreateFolder={onCreateFolder}
              />
            ))
          )}
        </div>
        <div className="flex shrink-0 items-center justify-end gap-2 border-t border-border px-4 py-3">
          <Button variant="ghost" size="sm" onClick={onClose}>
            {t('buttons.cancel')}
          </Button>
          <Button variant="primary" size="sm" onClick={() => onApply([...selected])}>
            {t('buttons.done')}
          </Button>
        </div>
      </div>
    </div>
  )
}
