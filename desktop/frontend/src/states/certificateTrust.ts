import { observable } from '@legendapp/state'
import { invoke } from '../lib/bridge'
import type { Account } from '../types'
import { errorMessage } from '../lib/errors'
import {
  certificateProbePayload,
  untrustedCertificateProtocol,
  type CertificateInfo,
  type CertificateProtocol,
  type CertificatePrompt,
} from '../lib/certificateTrust'
import { accounts$ } from './accounts'
import { showToast } from './ui'

// Certificate trust for an account that already exists. The account dialog
// handles this at setup time, where the user is looking at the servers they
// just typed; this covers the same failure arriving later — a message that
// cannot be submitted because the SMTP server's certificate changed, or an
// account added before it grew a self-signed one.

type CertPromptState = CertificatePrompt & { accountId: string }

export const certTrust$ = observable({
  prompt: null as CertPromptState | null,
  busy: false,
})

// Re-run after the pin is stored: the action that failed, retried once.
let pendingRetry: (() => void | Promise<void>) | null = null

/**
 * Offer to trust the certificate behind a failure, if that is what it was.
 * Returns true when the prompt is up and the caller should stay quiet — the
 * user is deciding, and the retry runs on acceptance. Returns false for any
 * other failure, which the caller reports as it always did.
 */
export async function offerCertificateTrust(
  accountId: string,
  message: string,
  retry: () => void | Promise<void>,
): Promise<boolean> {
  const protocol = untrustedCertificateProtocol(message)
  if (!protocol || !accountId) return false
  const account = accounts$.peek().find((candidate) => candidate.id === accountId)
  if (!account) return false
  const target = serverTarget(account, protocol)
  if (!target.host) return false
  try {
    const res = await invoke<{ certificate: CertificateInfo }>(
      'account.probeCert',
      // Same route as the connection that failed: an account on its own proxy
      // (or pinned to a direct connection while a global proxy is on) would
      // otherwise be probed over a different path — or not reached at all.
      certificateProbePayload({ ...target, protocol }, account.proxy),
    )
    if (!res?.certificate?.fingerprint) return false
    pendingRetry = retry
    certTrust$.prompt.set({ accountId, protocol, ...target, certificate: res.certificate })
    return true
  } catch {
    // The probe could not reach the server either. Nothing to show, so the
    // original failure stands.
    return false
  }
}

function serverTarget(account: Account, protocol: CertificateProtocol) {
  return protocol === 'smtp'
    ? { host: account.smtp_host, port: account.smtp_port || 465, starttls: !!account.smtp_starttls }
    : { host: account.imap_host, port: account.imap_port || 993, starttls: !!account.starttls }
}

/** Pin the accepted certificate on the account, then retry what failed. */
export async function trustPromptedCertificate() {
  const prompt = certTrust$.prompt.peek()
  if (!prompt) return
  const retry = pendingRetry
  certTrust$.busy.set(true)
  try {
    await invoke('account.setCertPin', {
      id: prompt.accountId,
      // Only the server the prompt was about: the other one keeps its pin.
      ...(prompt.protocol === 'smtp'
        ? { smtp_cert_pin: prompt.certificate.fingerprint }
        : { cert_pin: prompt.certificate.fingerprint }),
    })
    dismissCertificatePrompt()
    await retry?.()
  } catch (err) {
    // Leave the prompt up: the pin never landed, so retrying the send would
    // fail the same way.
    showToast(errorMessage(err, 'Could not save the certificate'), 'error')
  } finally {
    certTrust$.busy.set(false)
  }
}

export function dismissCertificatePrompt() {
  pendingRetry = null
  certTrust$.prompt.set(null)
}
