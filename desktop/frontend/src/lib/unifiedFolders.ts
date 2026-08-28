import type { Folder } from '../types'

/** The pseudo-account id the unified view selects. */
export const UNIFIED_ACCOUNT = 'unified'

/**
 * The folders the unified view offers, in the same order a single account lists
 * its own — so switching between an account and the unified view keeps the same
 * muscle memory. These are roles, not folder names: "Sent" here means each
 * account's own Sent, resolved per account by the core.
 */
export const UNIFIED_FOLDER_ROLES = ['inbox', 'starred', 'sent', 'drafts', 'archive', 'junk', 'trash'] as const

export type UnifiedFolderRole = (typeof UNIFIED_FOLDER_ROLES)[number]

export function isUnifiedFolderRole(value: string): value is UnifiedFolderRole {
  return (UNIFIED_FOLDER_ROLES as readonly string[]).includes(value)
}

/** The unified folder a bare/unknown selection falls back to. */
export const DEFAULT_UNIFIED_FOLDER: UnifiedFolderRole = 'inbox'

export function unifiedFolderRole(folderId: string): UnifiedFolderRole {
  return isUnifiedFolderRole(folderId) ? folderId : DEFAULT_UNIFIED_FOLDER
}

/**
 * Starred is the one unified folder with no per-account counterpart: it is a
 * flag, not a mailbox, so it is answered by a cross-account cache query rather
 * than the per-account folder fan-out. Its rows are ordinary thread cards, so
 * only the fetch differs — every list behaviour downstream is shared.
 */
export function isUnifiedStarred(accountId: string, folderId: string): boolean {
  return accountId === UNIFIED_ACCOUNT && folderId === 'starred'
}

/**
 * The folder in one account's own list that answers a unified role, or
 * undefined when that account's server has none — those accounts drop out of a
 * unified read rather than failing it. Listing resolves roles in the core; this
 * is for the callers that must name a real mailbox client-side, e.g. syncing
 * each account behind a unified Kanban column. Starred is never resolvable: it
 * is a flag, not a mailbox.
 */
export function accountFolderForRole(folders: Folder[] | undefined, role: UnifiedFolderRole): string | undefined {
  if (!folders?.length || role === 'starred') return undefined
  if (role === 'inbox') {
    return folders.find((folder) => folder.role === 'inbox' || folder.id.toLowerCase() === 'inbox')?.id
  }
  return folders.find((folder) => folder.role === role)?.id
}

const ROLE_LABEL_KEYS: Record<UnifiedFolderRole, string> = {
  inbox: 'folders.roles.inbox',
  // Reuses the filter label rather than minting a second "Starred" string.
  starred: 'filters.starred',
  sent: 'folders.roles.sent',
  drafts: 'folders.roles.drafts',
  archive: 'folders.roles.archive',
  junk: 'folders.roles.junk',
  trash: 'folders.roles.trash',
}

// A unified folder's name where it stands next to single-account ones: a kanban
// board, or the thread-list header and its folder picker. It has to carry
// "Unified" itself, since the plain role names ("Sent", "Trash") are the same
// ones an account shows. Spelled out per role rather than composed from
// "Unified" + a folder name, which no language agrees on.
const UNIFIED_LABEL_KEYS: Record<UnifiedFolderRole, string> = {
  inbox: 'kanban.columns.unifiedInbox',
  starred: 'kanban.columns.unifiedStarred',
  sent: 'kanban.columns.unifiedSent',
  drafts: 'kanban.columns.unifiedDrafts',
  archive: 'kanban.columns.unifiedArchive',
  junk: 'kanban.columns.unifiedJunk',
  trash: 'kanban.columns.unifiedTrash',
}

export function unifiedFolderLabel(folderId: string, t: (key: string) => string): string {
  return t(UNIFIED_LABEL_KEYS[unifiedFolderRole(folderId)])
}

/**
 * The unified view's synthetic folder list. Ids are the roles themselves, so
 * `ui$.selectedFolder` holds a role while the unified account is selected.
 * Only Inbox carries an unread count — it is the only one summed per account
 * by the folder-list load.
 */
export function unifiedFolders(t: (key: string) => string, inboxUnread = 0): Folder[] {
  return UNIFIED_FOLDER_ROLES.map((role) => ({
    id: role,
    account_id: UNIFIED_ACCOUNT,
    name: t(ROLE_LABEL_KEYS[role]),
    role,
    unread: role === 'inbox' ? inboxUnread : 0,
  }))
}
