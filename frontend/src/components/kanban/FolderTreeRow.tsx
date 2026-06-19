import { useEffect, useMemo, useRef, useState } from 'react'
import { ChevronRight, Folder as FolderIcon } from 'lucide-react'
import { Checkbox } from '../field/Checkbox'
import { collectKeys, type TreeNode } from './folderTree'

// One folder node in the column picker tree: an expander, a tri-state checkbox
// (covering the node and all descendants) and its children, rendered recursively.
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

  const descendantKeys = useMemo(() => collectKeys(node, accountId), [node, accountId])
  const checkedCount = descendantKeys.filter((key) => selected.has(key)).length
  const allChecked = descendantKeys.length > 0 && checkedCount === descendantKeys.length
  const someChecked = checkedCount > 0 && !allChecked

  useEffect(() => {
    if (checkboxRef.current) checkboxRef.current.indeterminate = someChecked
  }, [someChecked])

  const displayName =
    node.folder && (node.folder.id.toLowerCase() === 'inbox' || node.folder.name.toLowerCase() === 'inbox')
      ? isRSS
        ? 'Feed'
        : 'Inbox'
      : node.name

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
            disabled={descendantKeys.length === 0}
            onChange={(event) => onToggle(descendantKeys, event.target.checked)}
          />
          <FolderIcon size={14} className="shrink-0 text-secondary" />
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
