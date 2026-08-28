import { describe, expect, it } from 'bun:test'
import {
  DEFAULT_UNIFIED_FOLDER,
  UNIFIED_FOLDER_ROLES,
  isUnifiedStarred,
  unifiedFolderLabel,
  unifiedFolderRole,
  unifiedFolders,
} from './unifiedFolders'

const t = (key: string) => key

describe('unifiedFolders', () => {
  it('lists every role as a folder of the unified account', () => {
    const folders = unifiedFolders(t)
    expect(folders.map((folder) => folder.id)).toEqual([...UNIFIED_FOLDER_ROLES])
    expect(folders.every((folder) => folder.account_id === 'unified')).toBe(true)
    // The role drives the folder icon, so it has to survive onto the row.
    expect(folders.map((folder) => folder.role)).toEqual([...UNIFIED_FOLDER_ROLES])
  })

  it('counts unread on Inbox only', () => {
    const folders = unifiedFolders(t, 12)
    expect(folders.find((folder) => folder.id === 'inbox')?.unread).toBe(12)
    expect(folders.filter((folder) => folder.id !== 'inbox').every((folder) => folder.unread === 0)).toBe(true)
  })

  it('reuses the existing filter label for Starred rather than a second string', () => {
    expect(unifiedFolders(t).find((folder) => folder.id === 'starred')?.name).toBe('filters.starred')
  })
})

describe('unifiedFolderRole', () => {
  it('passes through known roles and falls back for anything else', () => {
    expect(unifiedFolderRole('sent')).toBe('sent')
    expect(unifiedFolderRole('trash')).toBe('trash')
    // A per-account folder id left over from the previously selected account
    // must not be sent to the core as a role.
    expect(unifiedFolderRole('[Gmail]/All Mail')).toBe(DEFAULT_UNIFIED_FOLDER)
    expect(unifiedFolderRole('')).toBe(DEFAULT_UNIFIED_FOLDER)
  })
})

describe('isUnifiedStarred', () => {
  it('is true only for the unified account showing its starred folder', () => {
    expect(isUnifiedStarred('unified', 'starred')).toBe(true)
    expect(isUnifiedStarred('unified', 'inbox')).toBe(false)
    // A real account can have a folder literally named "starred"; it is a normal
    // mailbox and must not take the cross-account path.
    expect(isUnifiedStarred('me@example.com', 'starred')).toBe(false)
  })
})

describe('unifiedFolderLabel', () => {
  const label = (key: string) =>
    ({
      'kanban.columns.unifiedInbox': 'Unified inbox',
      'kanban.columns.unifiedSent': 'Unified sent',
      'kanban.columns.unifiedDrafts': 'Unified drafts',
      'kanban.columns.unifiedArchive': 'Unified archive',
      'kanban.columns.unifiedJunk': 'Unified junk',
      'kanban.columns.unifiedTrash': 'Unified trash',
    })[key] ?? key

  it('names every unified role and falls back to inbox', () => {
    expect(unifiedFolderLabel('sent', label)).toBe('Unified sent')
    expect(unifiedFolderLabel('drafts', label)).toBe('Unified drafts')
    expect(unifiedFolderLabel('archive', label)).toBe('Unified archive')
    expect(unifiedFolderLabel('junk', label)).toBe('Unified junk')
    expect(unifiedFolderLabel('trash', label)).toBe('Unified trash')
    // A column persisted before unified folders were switchable, or one whose
    // stored role we no longer know, still names something.
    expect(unifiedFolderLabel('Receipts', label)).toBe('Unified inbox')
  })
})
