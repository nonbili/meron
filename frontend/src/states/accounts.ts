import { observable } from '@legendapp/state'
import { invoke } from '../lib/bridge'
import { boot } from '../boot'
import type { Account, Alias, ChatWallpaper } from '../types'
import { confirmAction, ui$, showToast } from './ui'

// Accounts state — maps 1:1 to the `accounts` DB table. The list is loaded by
// boot()/account flows; the mutations here update it optimistically and persist
// via the sidecar, reverting on failure.
export const accounts$ = observable([] as Account[])

// Accounts that participate in the unified inbox. Excluded accounts still exist
// and are reachable by selecting them directly; they just don't merge in.
export function unifiedAccounts() {
  return accounts$.get().filter((account) => account.included_in_unified !== false)
}

export function getActiveAccount() {
  const accounts = accounts$.get()
  const selected = ui$.selectedAccount.get()
  return accounts.find((account) => account.id === selected) ?? accounts[0] ?? null
}

export function isSendableAccount(acc: Account | undefined | null): boolean {
  return !!acc && acc.provider !== 'rss' && acc.auth_type !== 'rss' && acc.needs_reconnect !== true
}

// All send-as identities for an account: the primary address first (carrying the
// account's sender name), followed by each configured alias. Blank alias names
// inherit the account sender name, matching the backend send resolver.
export function accountIdentities(acc: Account): Alias[] {
  return [
    { email: acc.email, name: acc.sender_name },
    ...(acc.aliases ?? []).map((alias) => ({
      ...alias,
      name: alias.name?.trim() ? alias.name : acc.sender_name,
    })),
  ]
}

// Optimistically flip a boolean account field in local state, persist it via the
// given bridge method, and revert if the call fails. Backs the per-account
// toggles (images, unified-inbox inclusion, mute, pause).
async function setAccountFlag(
  accountId: string,
  field: 'load_remote_images' | 'conversation_html' | 'included_in_unified' | 'muted' | 'paused',
  method: string,
  enabled: boolean,
) {
  const previous = accounts$.get()
  accounts$.set(previous.map((acc) => (acc.id === accountId ? { ...acc, [field]: enabled } : acc)))
  try {
    await invoke(method, { id: accountId, enabled })
  } catch (error) {
    accounts$.set(previous)
    showToast(error instanceof Error ? error.message : 'Failed to update setting', 'error')
  }
}

// Toggle whether an account renders remote (URL-based) inline images.
export function setAccountImages(accountId: string, enabled: boolean) {
  return setAccountFlag(accountId, 'load_remote_images', 'account.setImages', enabled)
}

// Toggle whether conversation bubbles render original HTML when available.
export function setAccountConversationHtml(accountId: string, enabled: boolean) {
  return setAccountFlag(accountId, 'conversation_html', 'account.setConversationHtml', enabled)
}

// Toggle whether an account's inbox folds into the unified inbox.
export function setAccountUnified(accountId: string, enabled: boolean) {
  return setAccountFlag(accountId, 'included_in_unified', 'account.setUnified', enabled)
}

// Toggle whether an account raises desktop notifications for new mail. When
// muted, messages still arrive and the UI refreshes; only the OS toast is gone.
export function setAccountMuted(accountId: string, enabled: boolean) {
  return setAccountFlag(accountId, 'muted', 'account.setMuted', enabled)
}

// Pause/resume automatic checking for new messages. While paused, the sidecar
// stops the IDLE watcher and skips background syncs for the account.
export function setAccountPaused(accountId: string, enabled: boolean) {
  return setAccountFlag(accountId, 'paused', 'account.setPaused', enabled)
}

export async function setRSSSyncInterval(accountId: string, minutes: number) {
  const nextMinutes = Math.min(1440, Math.max(5, Math.round(minutes)))
  const previous = accounts$.get()
  accounts$.set(
    previous.map((acc) => (acc.id === accountId ? { ...acc, rss_sync_interval_minutes: nextMinutes } : acc)),
  )
  try {
    await invoke('account.setRSSSyncInterval', { id: accountId, minutes: nextMinutes })
  } catch (error) {
    accounts$.set(previous)
    showToast(error instanceof Error ? error.message : 'Failed to update sync interval', 'error')
  }
}

export async function setAccountChatWallpaper(accountId: string, wallpaper: ChatWallpaper | null) {
  const previous = accounts$.get()
  accounts$.set(previous.map((acc) => (acc.id === accountId ? { ...acc, chat_wallpaper: wallpaper } : acc)))
  try {
    await invoke('account.setChatWallpaper', { id: accountId, wallpaper })
  } catch (error) {
    accounts$.set(previous)
    showToast(error instanceof Error ? error.message : 'Failed to update chat background', 'error')
  }
}

export async function writeAccountChatWallpaperFile(
  accountId: string,
  file: { name: string; mime: string; data: string },
) {
  const res = await invoke<{ url: string }>('account.writeChatWallpaperFile', {
    id: accountId,
    filename: file.name,
    mime: file.mime,
    data: file.data,
  })
  return res.url
}

export async function deleteAccount(accountId: string) {
  if (
    !(await confirmAction({
      title: 'Remove account?',
      message: 'This account will be removed from Meron.',
      confirmLabel: 'Remove',
      tone: 'danger',
    }))
  ) {
    return
  }
  try {
    await invoke('account.remove', { id: accountId })
    showToast('Account removed')
    await boot()
  } catch (error) {
    showToast(error instanceof Error ? error.message : 'Failed to remove account', 'error')
  }
}

export async function reorderAccounts(oldIndex: number, newIndex: number) {
  const previous = accounts$.get()
  const accounts = [...previous]
  const [removed] = accounts.splice(oldIndex, 1)
  accounts.splice(newIndex, 0, removed)
  accounts$.set(accounts)
  try {
    await invoke('account.reorder', { accounts: accounts.map((acc) => acc.id) })
  } catch (error) {
    accounts$.set(previous)
    showToast(error instanceof Error ? error.message : 'Failed to reorder accounts', 'error')
  }
}

export async function reorderAccountIds(accountIds: string[]) {
  const previous = accounts$.get()
  const byId = new Map(previous.map((account) => [account.id, account]))
  const next = accountIds.flatMap((id) => {
    const account = byId.get(id)
    return account ? [account] : []
  })
  if (next.length !== previous.length) return
  accounts$.set(next)
  try {
    await invoke('account.reorder', { accounts: accountIds })
  } catch (error) {
    accounts$.set(previous)
    showToast(error instanceof Error ? error.message : 'Failed to reorder accounts', 'error')
  }
}

// Set a friendly name/label for an account.
export async function setAccountName(accountId: string, name: string) {
  const previous = accounts$.get()
  accounts$.set(previous.map((acc) => (acc.id === accountId ? { ...acc, display_name: name } : acc)))
  try {
    await invoke('account.setName', { id: accountId, name })
  } catch (error) {
    accounts$.set(previous)
    showToast(error instanceof Error ? error.message : 'Failed to update account name', 'error')
  }
}

// Set the sender name (outgoing mail From display name) for an account.
export async function setAccountSenderName(accountId: string, name: string) {
  const previous = accounts$.get()
  accounts$.set(previous.map((acc) => (acc.id === accountId ? { ...acc, sender_name: name } : acc)))
  try {
    await invoke('account.setSenderName', { id: accountId, name })
  } catch (error) {
    accounts$.set(previous)
    showToast(error instanceof Error ? error.message : 'Failed to update sender name', 'error')
  }
}

export async function writeAccountAvatarFile(accountId: string, file: { name: string; mime: string; data: string }) {
  const res = await invoke<{ url: string }>('account.writeAvatarFile', {
    id: accountId,
    filename: file.name,
    mime: file.mime,
    data: file.data,
  })
  return res.url
}

export async function setAccountAvatar(accountId: string, avatarUrl: string) {
  const previous = accounts$.get()
  const nextUrl = avatarUrl.trim()
  accounts$.set(previous.map((acc) => (acc.id === accountId ? { ...acc, avatar_url: nextUrl } : acc)))
  try {
    await invoke('account.setAvatar', { id: accountId, avatar_url: nextUrl })
    return true
  } catch (error) {
    accounts$.set(previous)
    showToast(error instanceof Error ? error.message : 'Failed to update account avatar', 'error')
    return false
  }
}

// Replace an account's send-as aliases (the whole list). The sidecar normalizes
// entries (trim, drop blank emails, dedupe by email).
export async function setAccountAliases(accountId: string, aliases: Alias[]) {
  const previous = accounts$.get()
  accounts$.set(previous.map((acc) => (acc.id === accountId ? { ...acc, aliases } : acc)))
  try {
    await invoke('account.setAliases', { id: accountId, aliases })
  } catch (error) {
    accounts$.set(previous)
    showToast(error instanceof Error ? error.message : 'Failed to update aliases', 'error')
  }
}
