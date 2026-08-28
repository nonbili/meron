import { describe, expect, it } from 'bun:test'
import {
  certificateProbePayload,
  commonName,
  formatFingerprint,
  isUntrustedCertificateError,
  shouldPromptForCertificate,
  untrustedCertificateProtocol,
} from './certificateTrust'

describe('certificateProbePayload', () => {
  const target = { host: '127.0.0.1', port: 1025, protocol: 'smtp' as const, starttls: true }

  it('keeps a reconnect probe on the account proxy', () => {
    const proxy = { mode: 'socks5' as const, host: '127.0.0.1', port: 9050 }
    expect(certificateProbePayload(target, proxy)).toEqual({ ...target, proxy })
  })

  it('follows the app proxy when the account has no override', () => {
    expect(certificateProbePayload(target)).toEqual({ ...target, proxy: null })
  })
})

describe('isUntrustedCertificateError', () => {
  it('recognizes the marker the core tags certificate failures with', () => {
    // What a Proton Mail Bridge account produces today.
    expect(
      isUntrustedCertificateError(
        'sidecar account.connect error: tls handshake: untrusted-certificate: invalid peer certificate: Other(OtherError(CaUsedAsEndEntity))',
      ),
    ).toBe(true)
  })

  it('leaves ordinary failures alone', () => {
    expect(isUntrustedCertificateError('tcp connect: connect 127.0.0.1:1143: connection refused')).toBe(false)
    expect(isUntrustedCertificateError('login failed: authentication failed')).toBe(false)
  })
})

describe('untrustedCertificateProtocol', () => {
  it('names the server that refused, so the right one is probed and pinned', () => {
    expect(untrustedCertificateProtocol('tls handshake: untrusted-certificate: bad cert')).toBe('imap')
    expect(untrustedCertificateProtocol('smtp-untrusted-certificate: bad cert')).toBe('smtp')
  })

  it('returns null for failures that are not about a certificate', () => {
    expect(untrustedCertificateProtocol('login failed: authentication failed')).toBe(null)
  })
})

describe('shouldPromptForCertificate', () => {
  it('allows a newly probed certificate to replace a stale reconnect pin', () => {
    expect(shouldPromptForCertificate('new-fingerprint', 'stale-fingerprint')).toBe(true)
  })

  it('does not prompt repeatedly for the pin already attempted by this save', () => {
    expect(shouldPromptForCertificate('ABCD', 'abcd')).toBe(false)
  })

  it('prompts when the failed request did not carry a pin', () => {
    expect(shouldPromptForCertificate('new-fingerprint')).toBe(true)
  })
})

describe('formatFingerprint', () => {
  it('prints byte pairs the way other tools do, for comparison', () => {
    expect(formatFingerprint('e3b0c44298fc1c14')).toBe('E3:B0:C4:42:98:FC:1C:14')
    expect(formatFingerprint('')).toBe('')
  })
})

describe('commonName', () => {
  it('picks the common name out of an RDN sequence', () => {
    expect(commonName('CN=127.0.0.1, O=Proton AG')).toBe('127.0.0.1')
    expect(commonName('C=CH, O=Proton AG, CN=Proton Mail Bridge')).toBe('Proton Mail Bridge')
  })

  it('falls back to the whole name when there is no CN', () => {
    expect(commonName('O=Proton AG')).toBe('O=Proton AG')
    expect(commonName('')).toBe('')
  })
})
