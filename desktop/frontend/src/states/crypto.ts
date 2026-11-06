// End-to-end encryption: bridge calls (meron-core `pgp.*`, see
// sidecar/dispatch/openpgp.rs) and the state of the encryption dialogs.
// Passphrases and key files pass through here and are never logged or kept.

import { observable } from '@legendapp/state'
import { invoke } from '../lib/bridge'
import { t } from '../lib/i18n'
import type {
  AccountEncryptionState,
  ImportOwnResult,
  ImportPreview,
  KeyInfo,
  KeySource,
  LookupResult,
  FoundKey,
  PeerRow,
  RecipientKeys,
} from '../lib/crypto'
import { loadThread } from './mail'
import { confirmAction, ui$ } from './ui'

/** A key file the user picked: base64, so binary `.gpg` keyrings work too. */
export type KeyFile = { name: string; data_b64: string }

/** Which key the verify dialog is open for. */
export type VerifyTarget = { addr: string; name: string; fingerprint: string; accountId: string }

export const crypto$ = observable({
  /** Account whose "turn on encryption" dialog is open. */
  enableFor: null as null | { accountId: string; address: string },
  verify: null as null | VerifyTarget,
  /** Addresses whose keys the lookup dialog is fetching (opened by a click). */
  lookup: null as null | { addrs: string[] },
  /** Bumped after any key change, so open settings pages reload. */
  version: 0,
})

function changed() {
  crypto$.version.set((v) => v + 1)
  // Indicators and trust are computed when a thread is read: re-read the open one.
  const open = ui$.selectedThread.get()
  if (open) void loadThread(open)
}

/** Opens the native picker for one key file. Null when cancelled. */
export async function pickKeyFile(title: string): Promise<KeyFile | null> {
  const res = await invoke<{ files: { name: string; data: string }[] } | { cancelled: true }>('system.pickFiles', {
    title,
  })
  if (!('files' in res) || res.files.length === 0) return null
  const file = res.files[0]
  const comma = file.data.indexOf(',')
  return { name: file.name, data_b64: comma >= 0 ? file.data.slice(comma + 1) : file.data }
}

export function accountEncryption(accountId: string) {
  return invoke<AccountEncryptionState>('pgp.account.state', { account: accountId })
}

export async function createKey(accountId: string) {
  const res = await invoke<{ key: KeyInfo }>('pgp.account.create', { account: accountId })
  changed()
  return res.key
}

export async function enableExisting(accountId: string) {
  const res = await invoke<{ enabled: boolean }>('pgp.account.enable', { account: accountId })
  changed()
  return res.enabled
}

export async function disableEncryption(accountId: string) {
  await invoke('pgp.account.disable', { account: accountId })
  changed()
}

export async function importOwnKey(accountId: string, file: KeyFile, passphrase?: string, fingerprint?: string) {
  const res = await invoke<{ result: ImportOwnResult; refetch?: number }>('pgp.account.importKey', {
    account: accountId,
    data_b64: file.data_b64,
    passphrase: passphrase ?? '',
    fingerprint,
  })
  if (res.result.status === 'imported') changed()
  return res
}

/** Exports go straight to a file the user picks (Go writes it, owner-only),
 * so key material never reaches the webview. */
export function exportPublicKey(accountId: string, filename: string) {
  return invoke<{ saved: boolean }>('pgp.account.exportPublic', { account: accountId, filename })
}

export function exportSecretKey(accountId: string, passphrase: string, filename: string) {
  return invoke<{ saved: boolean }>('pgp.account.exportSecret', { account: accountId, passphrase, filename })
}

export async function deleteOwnKey(accountId: string, fingerprint: string) {
  await invoke('pgp.account.deleteKey', { account: accountId, fingerprint })
  changed()
}

export function listPeers() {
  return invoke<{ peers: PeerRow[] }>('pgp.peers.list').then((r) => r.peers)
}

export function previewPeerImport(file: KeyFile) {
  return invoke<ImportPreview>('pgp.peers.importPreview', { data_b64: file.data_b64 })
}

export async function importPeers(file: KeyFile, fingerprints: string[]) {
  const res = await invoke<{ imported: number }>('pgp.peers.import', { data_b64: file.data_b64, fingerprints })
  changed()
  return res.imported
}

/** `confirm` is the fingerprint the user compared (pasted, or the shown one
 * after ticking "they match"); the core refuses if it differs. */
export async function setKeyVerified(addr: string, fingerprint: string, confirm: string, verified: boolean) {
  await invoke('pgp.peers.setVerified', { addr, fingerprint, confirm, verified })
  changed()
}

export async function removePeerKey(addr: string, fingerprint: string) {
  await invoke('pgp.peers.remove', { addr, fingerprint })
  changed()
}

/** "Not now" for one sender, or `*` for "Don't ask again for this account". */
export async function dismissEncryptionOffer(accountId: string, addr: string) {
  await invoke('pgp.offer.dismiss', { account: accountId, addr })
  changed()
}

export function recipientKeys(accountId: string, addrs: string[]) {
  return invoke<RecipientKeys>('pgp.keys.forRecipients', { account: accountId, addrs })
}

/** Before an encrypted send: when a recipient's key isn't the one Meron last
 * encrypted to for them (a new device, or someone else's key), asks
 * once whether to send anyway. Resolves true to go ahead. A failed check
 * doesn't stop the send: the core still refuses any recipient without a key. */
export async function confirmKeyChanges(accountId: string, addrs: string[]): Promise<boolean> {
  let changed: string[] = []
  try {
    changed = ((await recipientKeys(accountId, addrs)).changed ?? []).map((c) => c.addr)
  } catch {
    return true
  }
  if (changed.length === 0) return true
  const addresses = changed.join(', ')
  return confirmAction({
    title: t('crypto.keyChange.confirmTitle'),
    message: t('crypto.keyChange.confirmMessage', { addresses }),
    confirmLabel: t('crypto.keyChange.sendAnyway'),
  })
}

/** Looks an address up on the configured key sources. Only on a click: the
 * servers learn who the user writes to. */
export function lookupKey(addr: string) {
  return invoke<LookupResult>('pgp.lookup', { addr })
}

/** "Use this key": stored as Known (not Verified). */
export async function addLookedUpKey(addr: string, found: FoundKey) {
  const res = await invoke<{ key: KeyInfo }>('pgp.lookup.add', {
    addr,
    data_b64: found.data_b64,
    source: found.source.kind,
  })
  changed()
  return res.key
}

export function keySources() {
  return invoke<{ sources: KeySource[]; defaults: KeySource[] }>('pgp.lookup.sources')
}

export function setKeySources(sources: KeySource[]) {
  return invoke<{ sources: KeySource[] }>('pgp.lookup.setSources', { sources }).then((r) => r.sources)
}
