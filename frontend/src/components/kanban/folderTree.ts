import type { Folder } from '../../types'
import { kanbanColumnKey } from '../../states/kanban'

export type AccountGroup = {
  accountId: string
  label: string
  email?: string
  avatarUrl?: string
  isRSS: boolean
  folders: Folder[]
}

export type TreeNode = {
  /** Last path segment, shown as the label. */
  name: string
  /** Folder id (full IMAP path) when this node maps to a real folder. */
  folder?: Folder
  children: TreeNode[]
}

/** Pick the hierarchy delimiter: prefer the server-reported one, else infer. */
function pickDelimiter(folders: Folder[]): string {
  const reported = folders.find((folder) => folder.delimiter)?.delimiter
  if (reported) return reported
  if (folders.some((folder) => folder.name.includes('/'))) return '/'
  if (folders.some((folder) => folder.name.includes('.'))) return '.'
  return '/'
}

export function buildFolderTree(folders: Folder[]): TreeNode[] {
  const delimiter = pickDelimiter(folders)
  const roots: TreeNode[] = []
  const byPath = new Map<string, TreeNode>()

  for (const folder of folders) {
    const segments = delimiter ? folder.name.split(delimiter) : [folder.name]
    let parentChildren = roots
    let path = ''
    segments.forEach((segment, index) => {
      path = path ? `${path}${delimiter}${segment}` : segment
      let node = byPath.get(path)
      if (!node) {
        node = { name: segment, children: [] }
        byPath.set(path, node)
        parentChildren.push(node)
      }
      // The final segment is the real folder; intermediates may be structural.
      if (index === segments.length - 1) node.folder = folder
      parentChildren = node.children
    })
  }

  return roots
}

/** All selectable column keys reachable from a node (itself + descendants). */
export function collectKeys(node: TreeNode, accountId: string): string[] {
  const keys = node.folder ? [kanbanColumnKey({ accountId, folderId: node.folder.id })] : []
  for (const child of node.children) keys.push(...collectKeys(child, accountId))
  return keys
}
