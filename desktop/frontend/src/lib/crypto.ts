// End-to-end encryption (OpenPGP) as the UI sees it. The core decides every
// fact (meron-core `openpgp::mime::view_json`); this file only maps them to
// what is drawn: which icon, which colour, which strip.

export type Trust = 'verified' | 'known' | 'unconfirmed'

export type CryptoProblem =
  | 'no_key'
  | 'integrity'
  | 'too_large'
  | 'unreadable'
  | 'invalid_signature'
  | 'signer_mismatch'
  | 'weak_key_binding'
  | 'partly_encrypted'
  | 'inline_in_html'
  | 'key_revoked'
  | 'key_expired'
  | 'redirected'

/** A message's OpenPGP status, from `thread_message_json`. */
export type MessageCrypto = {
  encrypted: boolean
  signed: boolean
  trust: Trust
  /** Fingerprint of the key whose signature verified. */
  signer?: string | null
  /** Issuer named by a signature we couldn't verify. */
  issuer?: string | null
  partial: boolean
  weak_hash: boolean
  problem?: CryptoProblem | null
  key_changed: boolean
  /** A new key for a contact whose earlier key the user verified. */
  verified_conflict: boolean
  on_list: boolean
  /** Signed with one of the user's own keys. */
  own: boolean
  /** The signer's key is stored for the sender (so it can be verified). */
  signer_stored: boolean
  /** Show "turn on encryption" for this sender. */
  offer: boolean
}

/**
 * The status the core gives our own encrypted send once its Sent copy is
 * stored: encryption always signs with the account's own key, which the core
 * always trusts as verified (`Keyring::trust_of`). The bubble a send shows at
 * once carries it too, so its lock doesn't wait for the thread to reload.
 */
export function ownSentCrypto(): MessageCrypto {
  return {
    encrypted: true,
    signed: true,
    trust: 'verified',
    signer: null,
    issuer: null,
    partial: false,
    weak_hash: false,
    problem: null,
    key_changed: false,
    verified_conflict: false,
    on_list: false,
    own: true,
    signer_stored: true,
    offer: false,
  }
}

/** Icon shape says what the message is; tone says how far the key is trusted. */
export type IndicatorKind = 'lock' | 'signature'
export type IndicatorTone = 'verified' | 'known' | 'unconfirmed' | 'danger'

export type Indicator = { kind: IndicatorKind; tone: IndicatorTone }

const DECRYPT_FAILURES: CryptoProblem[] = ['no_key', 'integrity', 'too_large', 'unreadable']
const DANGER: CryptoProblem[] = [
  ...DECRYPT_FAILURES,
  'invalid_signature',
  'signer_mismatch',
  'key_revoked',
  'key_expired',
  'redirected',
]

/**
 * The header indicator for a message, or null for ordinary mail (plain,
 * unsigned mail shows nothing at all).
 */
export function indicatorFor(crypto?: MessageCrypto | null): Indicator | null {
  if (!crypto || (!crypto.encrypted && !crypto.signed)) return null
  const kind: IndicatorKind = crypto.encrypted ? 'lock' : 'signature'
  if (crypto.problem && DANGER.includes(crypto.problem)) return { kind, tone: 'danger' }
  if (crypto.verified_conflict) return { kind, tone: 'danger' }
  if (crypto.problem === 'weak_key_binding') return { kind, tone: 'unconfirmed' }
  return { kind, tone: crypto.trust }
}

/** Colour per tone. Verified is a calm green (not neon, but clearly green), defined
 * once as `--color-verified` in index.css for light and dark; amber and rose are
 * Meron's warning and error colours. */
export const TONE_CLASS: Record<IndicatorTone, string> = {
  verified: 'text-verified',
  known: 'text-secondary',
  unconfirmed: 'text-amber-600 dark:text-amber-400',
  danger: 'text-rose-600 dark:text-rose-400',
}

export type StripKind =
  | { kind: 'problem'; problem: CryptoProblem; danger: boolean }
  | { kind: 'key_changed'; danger: boolean }
  | { kind: 'offer' }

/** What, if anything, goes in the strip above the body. One at a time,
 * most important first. */
export function stripFor(crypto?: MessageCrypto | null): StripKind | null {
  if (!crypto) return null
  // A weak binding must not hide a changed sender key or a verified-key conflict.
  if (crypto.problem === 'weak_key_binding' && (crypto.key_changed || crypto.verified_conflict)) {
    return { kind: 'key_changed', danger: crypto.verified_conflict }
  }
  if (crypto.problem) {
    const neutral = crypto.problem === 'inline_in_html' || crypto.problem === 'weak_key_binding'
    return { kind: 'problem', problem: crypto.problem, danger: !neutral }
  }
  if (crypto.key_changed) return { kind: 'key_changed', danger: crypto.verified_conflict }
  if (crypto.offer) return { kind: 'offer' }
  return null
}

/** Whether decryption failed, so there is no body to show. */
export function decryptFailed(crypto?: MessageCrypto | null): boolean {
  return !!crypto?.problem && DECRYPT_FAILURES.includes(crypto.problem)
}

/** Upper-case hex, no spaces: the form fingerprints are compared in. */
export function normalizeFingerprint(value: string): string {
  return value.replace(/[^0-9a-fA-F]/g, '').toUpperCase()
}

/** 40 hex digits as ten groups of four, the way users read them aloud. */
export function fingerprintGroups(fingerprint: string): string[] {
  return normalizeFingerprint(fingerprint).match(/.{1,4}/g) ?? []
}

export function formatFingerprint(fingerprint: string): string {
  return fingerprintGroups(fingerprint).join(' ')
}

/** The last 16 hex digits, enough to tell keys apart in a list. */
export function shortFingerprint(fingerprint: string): string {
  return fingerprintGroups(normalizeFingerprint(fingerprint).slice(-16)).join(' ')
}

/** A pasted fingerprint matches the key's (spaces, case and separators ignored). */
export function fingerprintsMatch(pasted: string, fingerprint: string): boolean {
  const a = normalizeFingerprint(pasted)
  return a.length >= 32 && a === normalizeFingerprint(fingerprint)
}

/** Key facts from the core (`openpgp::keys::KeyInfo`). */
export type KeyInfo = {
  fingerprint: string
  user_ids: string[]
  emails: string[]
  created: number
  expires?: number | null
  algorithm: string
  has_secret: boolean
  can_encrypt: boolean
  supports_seipd_v2: boolean
  unusable?: 'revoked' | 'expired' | 'invalid_bindings' | 'no_encryption_key' | 'secret_on_card' | null
}

export type AccountEncryptionState = {
  enabled: boolean
  key?: KeyInfo | null
  previous: KeyInfo[]
}

export type PeerKeySource = 'autocrypt' | 'gossip' | 'attachment' | 'imported' | 'wkd' | 'keyserver'

export type PeerRow = {
  addr: string
  source: PeerKeySource
  verified: boolean
  first_seen: number
  last_seen: number
  info: KeyInfo
  conflicts_with_verified: boolean
}

export type ImportPreview = {
  ready: KeyInfo[]
  skipped: { info: KeyInfo; reason: 'expired' | 'revoked' | 'invalid_bindings' | 'no_encryption_key' | 'no_email' }[]
  already_present: KeyInfo[]
  replaces_unverified: string[]
  conflicts_with_verified: string[]
  /** Stored keys this file updates (revocations, renewals); applied on import. */
  updates: KeyInfo[]
  unreadable: number
}

export type ImportOwnResult =
  | { status: 'imported'; key: KeyInfo; address_matches: boolean }
  | { status: 'choose'; keys: KeyInfo[] }
  | { status: 'needs_passphrase'; key: KeyInfo }
  | { status: 'on_card' }
  | { status: 'no_secret_key' }

export type MissingKey = { addr: string; reason: 'none' | 'unusable' | 'new_key_unverified' }

/** A recipient whose key isn't the one Meron last encrypted to for them. */
export type KeyChange = { addr: string; previous: string; current: string }

export type RecipientKeys = {
  enabled: boolean
  own_key_unusable: boolean
  missing: MissingKey[]
  /** Keys that changed since the last encrypted mail to that address. */
  changed?: KeyChange[]
}

/** Whether a decrypted message's quote may go into an unencrypted reply. It
 * may not: quoting it would send the plaintext in the clear ("Re: What's up
 * Johnny?", security notes §4). */
export function mayQuoteInto(original: { crypto?: MessageCrypto | null } | null | undefined, encrypting: boolean) {
  return encrypting || !original?.crypto?.encrypted
}

/** Whether a reply may quote this message at all. A decrypted message is
 * quoted only when a valid signature by a key bound to its From address covers
 * it (trust known or verified). Otherwise anyone holding an old ciphertext can
 * resend it from their own address and read the plaintext in our reply, which
 * is encrypted to *them* ("Re: What's up Johnny?"): unsigned,
 * signed by someone else, by an unknown key, partly signed or weak-hash all
 * come out `unconfirmed` from the core (`CryptoStatus::trust`). */
export function mayQuoteOriginal(crypto?: MessageCrypto | null) {
  return !crypto?.encrypted || crypto.trust !== 'unconfirmed'
}

/** Where key lookups go (meron-core `openpgp::lookup`). */
export type KeySourceKind = 'wkd' | 'vks' | 'hkp'
export type KeySource = { kind: KeySourceKind; url: string; enabled: boolean }

export type FoundKey = { source: KeySource; verified: boolean; info: KeyInfo; data_b64: string }
export type LookupResult = {
  addr: string
  found: FoundKey[]
  searched: string[]
  failed: { source: string; error: string }[]
  /** Stored keys this lookup updated (a revocation or renewal found online). */
  updated?: KeyInfo[]
}

/** A source's name for the UI: WKD, or the keyserver's host. */
export function sourceLabel(source: KeySource): string {
  return source.kind === 'wkd' ? 'WKD' : source.url.replace(/^https:\/\//, '').replace(/\/$/, '')
}
