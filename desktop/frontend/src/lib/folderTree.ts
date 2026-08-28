import type { Folder } from '../types'

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
export function pickDelimiter(folders: Folder[]): string {
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

/** All real folder ids reachable from a node (itself + descendants). */
export function collectFolderIds(node: TreeNode): string[] {
  const ids = node.folder ? [node.folder.id] : []
  for (const child of node.children) ids.push(...collectFolderIds(child))
  return ids
}

/** Folder ids controlled by a node's checkbox. Real folders select independently;
 * synthetic path nodes group-select the real folders beneath them. */
export function selectableFolderIds(node: TreeNode): string[] {
  return node.folder ? [node.folder.id] : collectFolderIds(node)
}
