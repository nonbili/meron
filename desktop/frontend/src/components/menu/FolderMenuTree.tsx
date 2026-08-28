import { folderIcon } from '../../lib/folderIcon'
import { clsx } from '../../lib/utils'
import { buildFolderTree, type TreeNode } from '../../lib/folderTree'
import type { Folder } from '../../types'
import { menuItemBase } from './menuStyles'

/** Does this subtree contain anything worth clicking? */
function hasPickable(node: TreeNode, excluded: Set<string>): boolean {
  if (node.folder && !excluded.has(node.folder.id)) return true
  return node.children.some((child) => hasPickable(child, excluded))
}

function FolderMenuRow({
  node,
  depth,
  excluded,
  onPick,
}: {
  node: TreeNode
  depth: number
  excluded: Set<string>
  onPick: (folder: Folder) => void
}) {
  const folder = node.folder
  const pickable = !!folder && !excluded.has(folder.id)
  const Icon = folderIcon(folder)

  return (
    <div>
      <button
        type="button"
        disabled={!pickable}
        // Structural segments and the folder the thread already lives in stay
        // visible so the hierarchy reads correctly, but can't be picked.
        className={clsx(menuItemBase, pickable ? 'text-primary hover:bg-hover' : 'cursor-default text-secondary')}
        style={{ paddingLeft: depth * 14 + 8 }}
        onClick={() => folder && onPick(folder)}
      >
        <Icon size={13} className="shrink-0 text-secondary" />
        <span className="min-w-0 truncate">{node.name}</span>
      </button>
      {node.children.map((child) =>
        hasPickable(child, excluded) ? (
          <FolderMenuRow
            key={child.folder?.id ?? `${depth}-${child.name}`}
            node={child}
            depth={depth + 1}
            excluded={excluded}
            onPick={onPick}
          />
        ) : null,
      )}
    </div>
  )
}

/**
 * A flyout's folder list rendered as the account's folder hierarchy: nested
 * folders are indented under their parent instead of showing their full path.
 */
export function FolderMenuTree({
  folders,
  excludedFolderIds,
  onPick,
}: {
  folders: Folder[]
  /** Folders shown as context but not selectable (the current folder, etc.). */
  excludedFolderIds?: Iterable<string>
  onPick: (folder: Folder) => void
}) {
  const excluded = new Set(excludedFolderIds ?? [])
  return (
    <>
      {buildFolderTree(folders).map((node) =>
        hasPickable(node, excluded) ? (
          <FolderMenuRow key={node.folder?.id ?? node.name} node={node} depth={0} excluded={excluded} onPick={onPick} />
        ) : null,
      )}
    </>
  )
}
