import { beforeEach, describe, expect, it } from 'bun:test'
import { certTrust$, dismissCertificatePrompt, trustPromptedCertificate } from './certificateTrust'
import { retrySend } from './compose'
import { setPendingSend, getPendingSend, type PendingSend } from './pendingSends'
import { accounts$ } from './accounts'
import { mail$ } from './mail'

const BRIDGE_CERT_ERROR = 'smtp-untrusted-certificate: invalid peer certificate: Other(OtherError(CaUsedAsEndEntity))'

const pending = (): PendingSend => ({
  account_id: 'acc-bridge',
  to: 'someone@example.com',
  cc: '',
  subject: 'Hi',
  body: 'Ping',
  in_reply_to: '',
  references: '',
  from: 'me@example.com',
  message_id: '<m1@meron>',
  attachments: [],
})

describe('certificate trust on the send path', () => {
  const calls: { command: string; payload: any }[] = []
  let sendFailures = 1

  beforeEach(() => {
    calls.length = 0
    sendFailures = 1
    dismissCertificatePrompt()
    mail$.messages.set([])
    accounts$.set([
      {
        id: 'acc-bridge',
        email: 'me@example.com',
        display_name: 'Me',
        provider: 'custom',
        auth_type: 'password',
        imap_host: '127.0.0.1',
        imap_port: 1143,
        smtp_host: '127.0.0.1',
        smtp_port: 1025,
        tls: false,
        starttls: true,
        smtp_starttls: true,
        proxy: { mode: 'socks5', host: '127.0.0.1', port: 9050 },
      },
    ] as any)
    setPendingSend('sent-1', pending())
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: any) => {
            calls.push({ command, payload })
            if (command === 'mail.send' && sendFailures-- > 0) throw new Error(BRIDGE_CERT_ERROR)
            if (command === 'account.probeCert') {
              return {
                certificate: {
                  fingerprint: '6f69d6a7',
                  subject: 'CN=127.0.0.1, O=Proton Mail Bridge',
                  issuer: 'CN=127.0.0.1, O=Proton Mail Bridge',
                  not_before: 'Sat, 22 Aug 2026 23:57:04 +0000',
                  not_after: 'Mon, 24 Aug 2026 23:57:04 +0000',
                  self_signed: true,
                },
              }
            }
            return { ok: true }
          },
        },
      },
    }
  })

  it('offers the submission server certificate instead of a dead-end failure', async () => {
    await retrySend('sent-1')

    // Probed the SMTP server the account sends through, not its IMAP server.
    expect(calls.find((call) => call.command === 'account.probeCert')?.payload).toEqual({
      host: '127.0.0.1',
      port: 1025,
      protocol: 'smtp',
      starttls: true,
      // Same route as the send that failed, not the app-wide proxy.
      proxy: { mode: 'socks5', host: '127.0.0.1', port: 9050 },
    })
    const prompt = certTrust$.prompt.get()
    expect(prompt?.protocol).toBe('smtp')
    expect(prompt?.certificate.fingerprint).toBe('6f69d6a7')
    // The send is still pending: it failed, and the retry waits on the user.
    expect(getPendingSend('sent-1')).toBeTruthy()
  })

  it('pins only the accepted server and retries the send', async () => {
    await retrySend('sent-1')
    await trustPromptedCertificate()

    const pin = calls.find((call) => call.command === 'account.setCertPin')
    expect(pin?.payload).toEqual({ id: 'acc-bridge', smtp_cert_pin: '6f69d6a7' })
    // Retried once the pin landed, and the second attempt went through.
    expect(calls.filter((call) => call.command === 'mail.send')).toHaveLength(2)
    expect(getPendingSend('sent-1')).toBeUndefined()
    expect(certTrust$.prompt.get()).toBe(null)
  })

  it('leaves ordinary send failures alone', async () => {
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      throw new Error('smtp auth: authentication failed')
    }

    await retrySend('sent-1')

    expect(calls.some((call) => call.command === 'account.probeCert')).toBe(false)
    expect(certTrust$.prompt.get()).toBe(null)
  })
})
