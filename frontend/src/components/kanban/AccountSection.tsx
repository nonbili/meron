import { useEffect, useRef, useState } from 'react'
import type { MouseEvent } from 'react'
import { ChevronRight, Loader2, Plus, X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import type { Folder } from '../../types'
import { kanbanColumnKey } from '../../states/kanban'
import { Avatar } from '../avatar/Avatar'
import { IconButton } from '../button/IconButton'
import { TextInput } from '../field/Field'
import { FolderTreeRow } from './FolderTreeRow'
import type { AccountGroup, TreeNode } from './folderTree'

// One account's collapsible section in the column picker: an inline "new folder"
// form (for non-RSS accounts) and its folder tree.
export function AccountSection({
  group,
  selected,
  onToggle,
  onCreateFolder,
}: {
  group: AccountGroup & { tree: TreeNode[] }
  selected: Set<string>
  onToggle: (keys: string[], next: boolean) => void
  onCreateFolder?: (accountId: string, name: string) => Promise<Folder>
}) {
  const { t } = useTranslation()
  const [expanded, setExpanded] = useState(true)
  const [createOpen, setCreateOpen] = useState(false)
  const [name, setName] = useState('')
  const [error, setError] = useState('')
  const [creating, setCreating] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (createOpen) inputRef.current?.focus()
  }, [createOpen])

  async function createFolder() {
    if (!onCreateFolder || group.isRSS || creating || !createOpen) return
    const trimmed = name.trim()
    if (!trimmed) {
      setError(t('folders.nameRequired'))
      return
    }
    const duplicate = group.folders.some((folder) => folder.id === trimmed || folder.name === trimmed)
    if (duplicate) {
      setError(t('folders.alreadyExists'))
      return
    }
    setCreating(true)
    setError('')
    try {
      const folder = await onCreateFolder(group.accountId, trimmed)
      onToggle([kanbanColumnKey({ accountId: group.accountId, folderId: folder.id })], true)
      setName('')
      setCreateOpen(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : t('folders.createFailed'))
    } finally {
      setCreating(false)
    }
  }

  function openCreate(event: MouseEvent) {
    event.stopPropagation()
    setExpanded(true)
    setCreateOpen(true)
    setError('')
  }

  function closeCreate() {
    if (creating) return
    setCreateOpen(false)
    setName('')
    setError('')
  }

  return (
    <div className="mb-1">
      <div className="flex items-center gap-1 rounded-lg px-2 pb-1 pt-2 hover:bg-hover">
        <button
          type="button"
          className="flex min-w-0 flex-1 cursor-pointer items-center gap-1 text-left text-[10px] font-bold uppercase tracking-wide text-secondary hover:text-primary"
          onClick={() => setExpanded((open) => !open)}
          title={expanded ? t('accounts.actions.collapseAccount') : t('accounts.actions.expandAccount')}
        >
          <ChevronRight size={13} className={`shrink-0 transition-transform ${expanded ? 'rotate-90' : ''}`} />
          <Avatar name={group.label} email={group.email} src={group.avatarUrl} size={18} />
          <span className="truncate">{group.label}</span>
        </button>
        {!group.isRSS && onCreateFolder && (
          <IconButton
            icon={Plus}
            label={t('folders.newFolderOrLabel')}
            size="sm"
            radius="lg"
            variant={createOpen ? 'accentSoft' : 'ghost'}
            active={createOpen}
            onClick={openCreate}
          />
        )}
      </div>
      {expanded && (
        <div>
          {!group.isRSS && onCreateFolder && createOpen && (
            <form
              className="mx-2 mb-1 flex flex-col gap-1"
              onSubmit={(event) => {
                event.preventDefault()
                void createFolder()
              }}
            >
              <div className="flex items-center gap-1.5 rounded-lg bg-hover p-1">
                <TextInput
                  ref={inputRef}
                  value={name}
                  onChange={(event) => {
                    setName(event.target.value)
                    if (error) setError('')
                  }}
                  placeholder={t('folders.namePlaceholder')}
                  disabled={creating}
                  className="h-7 flex-1 rounded-md px-2 font-medium"
                />
                <IconButton
                  icon={creating ? undefined : Plus}
                  label={t('folders.create')}
                  size="sm"
                  radius="lg"
                  variant="accentSoft"
                  disabled={creating}
                  type="submit"
                >
                  {creating && <Loader2 size={14} className="animate-spin" />}
                </IconButton>
                <IconButton
                  icon={X}
                  label={t('buttons.cancel')}
                  size="sm"
                  radius="lg"
                  onClick={closeCreate}
                  disabled={creating}
                />
              </div>
              {error && <div className="mt-1.5 px-1 text-[11px] font-medium text-rose-500">{error}</div>}
            </form>
          )}
          {group.tree.map((node) => (
            <FolderTreeRow
              key={node.folder?.id ?? node.name}
              node={node}
              accountId={group.accountId}
              isRSS={group.isRSS}
              depth={0}
              selected={selected}
              onToggle={onToggle}
            />
          ))}
        </div>
      )}
    </div>
  )
}
