import {
  Archive,
  FileText,
  Folder as FolderGeneric,
  Inbox,
  Mailbox,
  Mails,
  OctagonAlert,
  Send,
  StickyNote,
  Star,
  Trash2,
  type LucideIcon,
} from 'lucide-react'
import type { Folder } from '../types'

const ROLE_ICONS: Record<string, LucideIcon> = {
  inbox: Inbox,
  sent: Send,
  drafts: FileText,
  trash: Trash2,
  junk: OctagonAlert,
  archive: Archive,
  all: Mails,
  // Not an IMAP special-use: the unified view's cross-account Starred folder.
  starred: Star,
}

// Servers that don't advertise RFC 6154 special-use still name these folders
// predictably, so fall back to the name. Kept to exact well-known names: a
// user folder called "Archives" or "Notebook" should stay a plain folder.
const NAME_ICONS: Record<string, LucideIcon> = {
  inbox: Inbox,
  sent: Send,
  'sent mail': Send,
  'sent items': Send,
  'sent messages': Send,
  drafts: FileText,
  draft: FileText,
  outbox: Mailbox,
  trash: Trash2,
  bin: Trash2,
  deleted: Trash2,
  'deleted items': Trash2,
  'deleted messages': Trash2,
  junk: OctagonAlert,
  spam: OctagonAlert,
  'junk e-mail': OctagonAlert,
  'junk email': OctagonAlert,
  'bulk mail': OctagonAlert,
  archive: Archive,
  'all mail': Mails,
  notes: StickyNote,
  starred: Star,
  flagged: Star,
}

/**
 * Icon for a folder: its special-use role when the server advertises one,
 * otherwise a guess from the folder's own name, otherwise a plain folder.
 */
export function folderIcon(folder?: Pick<Folder, 'id' | 'name' | 'role'> | null): LucideIcon {
  if (!folder) return FolderGeneric
  const byRole = folder.role && ROLE_ICONS[folder.role.toLowerCase()]
  if (byRole) return byRole
  // Hierarchical ids arrive as full paths ("INBOX.Sent", "[Gmail]/Trash"); only
  // the leaf carries the meaning.
  const leaf = (folder.name || folder.id).split(/[/.]/).pop() ?? ''
  return NAME_ICONS[leaf.trim().toLowerCase()] ?? FolderGeneric
}
