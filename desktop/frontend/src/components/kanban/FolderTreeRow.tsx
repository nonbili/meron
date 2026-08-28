import { useEffect, useMemo, useRef, useState } from 'react'
import { ChevronRight, Rss } from 'lucide-react'
import { folderIcon } from '../../lib/folderIcon'
import { kanbanColumnKey } from '../../states/kanban'
import { Checkbox } from '../field/Checkbox'
import { selectableFolderIds, type TreeNode } from '../../lib/folderTree'

// One folder node in the column picker tree: an expander, a checkbox for the
// folder itself, and its children, rendered recursively. A structural node
// without a real folder keeps a tri-state checkbox covering its descendants.
export function FolderTreeRow({
  node,
  accountId,
  isRSS,
  depth,
  selected,
  onToggle,
}: {
  node: TreeNode
  accountId: string
  isRSS: boolean
  depth: number
  selected: Set<string>
  onToggle: (keys: string[], next: boolean) => void
}) {
  const [expanded, setExpanded] = useState(true)
  const checkboxRef = useRef<HTMLInputElement>(null)
  const hasChildren = node.children.length > 0

  const controlledKeys = useMemo(
    () => selectableFolderIds(node).map((folderId) => kanbanColumnKey({ accountId, folderId })),
    [node, accountId],
  )
  const checkedCount = controlledKeys.filter((key) => selected.has(key)).length
  const allChecked = controlledKeys.length > 0 && checkedCount === controlledKeys.length
  const someChecked = checkedCount > 0 && !allChecked

  useEffect(() => {
    if (checkboxRef.current) checkboxRef.current.indeterminate = someChecked
  }, [someChecked])

  const isInbox =
    !!node.folder && (node.folder.id.toLowerCase() === 'inbox' || node.folder.name.toLowerCase() === 'inbox')
  const displayName = isInbox ? (isRSS ? 'Feed' : 'Inbox') : node.name
  const Icon = isRSS && isInbox ? Rss : folderIcon(node.folder)

  return (
    <div>
      <div
        className="flex items-center gap-1 rounded-lg py-1.5 pr-2 hover:bg-hover"
        style={{ paddingLeft: depth * 18 + 4 }}
      >
        <button
          type="button"
          className={`flex h-5 w-5 shrink-0 items-center justify-center rounded text-secondary ${
            hasChildren ? 'cursor-pointer hover:text-primary' : 'invisible'
          }`}
          onClick={() => setExpanded((open) => !open)}
          tabIndex={hasChildren ? 0 : -1}
        >
          <ChevronRight size={14} className={`transition-transform ${expanded ? 'rotate-90' : ''}`} />
        </button>
        <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-2">
          <Checkbox
            ref={checkboxRef}
            checked={allChecked}
            disabled={controlledKeys.length === 0}
            onChange={(event) => onToggle(controlledKeys, event.target.checked)}
          />
          <Icon size={14} className="shrink-0 text-secondary" />
          <span className="truncate text-xs font-semibold text-primary">{displayName}</span>
        </label>
      </div>
      {hasChildren && expanded && (
        <div>
          {node.children.map((child) => (
            <FolderTreeRow
              key={child.folder?.id ?? `${depth}-${child.name}`}
              node={child}
              accountId={accountId}
              isRSS={isRSS}
              depth={depth + 1}
              selected={selected}
              onToggle={onToggle}
            />
          ))}
        </div>
      )}
    </div>
  )
}
