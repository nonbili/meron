import type { AccountProxy } from '../types'

// Trust-on-first-use for mail servers whose certificate cannot be validated
// against the public roots — a local Proton Mail Bridge, say, which serves a
// self-signed CA certificate as its leaf. The core tags those handshake
// failures, we fetch the certificate it was rejecting, and the user decides
// whether to pin it.

export type CertificateInfo = {
  fingerprint: string
  subject: string
  issuer: string
  not_before: string
  not_after: string
  self_signed: boolean
}

// Which of an account's two servers a prompt is about. They can be different
// daemons with different certificates, so each carries its own pin.
export type CertificateProtocol = 'imap' | 'smtp'

export type CertificatePrompt = {
  host: string
  port: number
  protocol: CertificateProtocol
  certificate: CertificateInfo
}

export type CertificateTarget = Omit<CertificatePrompt, 'certificate'>

// Keep every certificate probe on the same route as the connection it is
// inspecting. An absent account override explicitly follows the app-wide proxy.
export function certificateProbePayload(target: CertificateTarget, proxy?: AccountProxy | null) {
  return { ...target, proxy: proxy ?? null }
}

// Must match the markers in meron-core/src/tls.rs. The SMTP one contains the
// general one, so an untrusted certificate matches whichever check runs first.
const UNTRUSTED_CERT_MARKER = 'untrusted-certificate'
const UNTRUSTED_SMTP_CERT_MARKER = 'smtp-untrusted-certificate'

export function isUntrustedCertificateError(message: string): boolean {
  return message.includes(UNTRUSTED_CERT_MARKER)
}

// The server a failed save was talking to when it rejected the certificate:
// IMAP is validated first, the submission server right after it.
export function untrustedCertificateProtocol(message: string): CertificateProtocol | null {
  if (message.includes(UNTRUSTED_SMTP_CERT_MARKER)) return 'smtp'
  return isUntrustedCertificateError(message) ? 'imap' : null
}

// Re-prompt only when the probe found a certificate different from the pin the
// failed request actually tried. This lets reconnect replace a stale stored pin
// without looping when a pin accepted during this save somehow still fails.
export function shouldPromptForCertificate(fingerprint: string, attemptedPin?: string): boolean {
  return fingerprint.trim().toLowerCase() !== (attemptedPin ?? '').trim().toLowerCase()
}

// Hex SHA-256 as colon-separated byte pairs, the form every other tool
// (openssl, browsers, Bridge's own logs) prints so the two can be compared.
export function formatFingerprint(fingerprint: string): string {
  const hex = fingerprint.trim().toUpperCase()
  return (hex.match(/.{1,2}/g) ?? []).join(':')
}

// X.509 names arrive as an RDN string ("CN=127.0.0.1, O=Proton AG"). Show the
// common name when there is one; the full sequence is noise in a dialog.
export function commonName(name: string): string {
  const match = name.match(/(?:^|,)\s*CN=([^,]+)/i)
  return (match ? match[1] : name).trim()
}
