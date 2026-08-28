import { ShieldAlert } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { commonName, formatFingerprint, type CertificatePrompt } from '../../lib/certificateTrust'
import { Button } from '../button/Button'

// Shown when a server's certificate cannot be validated against the public
// roots — a local Proton Mail Bridge, say. The user compares the fingerprint
// against the one the server is supposed to have and decides; accepting pins
// that exact certificate for that account's server, and nothing else.
//
// Used inline by the account dialog (where a save was refused) and inside a
// modal by the send path (where a message could not be submitted).
export function CertificateTrustPanel({
  prompt,
  busy = false,
  onTrust,
  onDismiss,
}: {
  prompt: CertificatePrompt
  busy?: boolean
  onTrust: () => void
  onDismiss: () => void
}) {
  const { t } = useTranslation()
  const cert = prompt.certificate
  const rows: [string, string][] = [
    [t('accounts.certificate.issuedTo', { defaultValue: 'Issued to' }), commonName(cert.subject)],
    [t('accounts.certificate.issuedBy', { defaultValue: 'Issued by' }), commonName(cert.issuer)],
    [t('accounts.certificate.expires', { defaultValue: 'Expires' }), cert.not_after],
  ]
  return (
    <div className="rounded-xl bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-900/50 p-3 flex flex-col gap-2.5">
      <div className="flex items-start gap-2">
        <ShieldAlert size={14} className="shrink-0 mt-px text-amber-600 dark:text-amber-400" />
        <div className="flex flex-col gap-1">
          <p className="text-[0.75rem] font-bold leading-tight text-amber-700 dark:text-amber-300">
            {t('accounts.certificate.title', { defaultValue: "Can't verify this server's certificate" })}
          </p>
          <p className="text-[0.6875rem] leading-relaxed text-amber-700/90 dark:text-amber-300/90">
            {t('accounts.certificate.body', {
              defaultValue:
                'Check the fingerprint below against the one {server} is supposed to have. Only continue if they match.',
              server: `${prompt.host}:${prompt.port}`,
            })}
          </p>
        </div>
      </div>
      <dl className="flex flex-col gap-1 text-[0.6875rem] text-amber-700/90 dark:text-amber-300/90">
        {rows.map(([label, value]) =>
          value ? (
            <div key={label} className="flex gap-2">
              <dt className="w-20 shrink-0 font-medium">{label}</dt>
              <dd className="min-w-0 break-words">{value}</dd>
            </div>
          ) : null,
        )}
        <div className="flex gap-2">
          <dt className="w-20 shrink-0 font-medium">
            {t('accounts.certificate.fingerprint', { defaultValue: 'SHA-256' })}
          </dt>
          <dd className="min-w-0 font-mono break-all">{formatFingerprint(cert.fingerprint)}</dd>
        </div>
      </dl>
      <div className="flex gap-2 justify-end">
        <Button variant="ghost" size="sm" disabled={busy} onClick={onDismiss}>
          {t('buttons.cancel')}
        </Button>
        <Button size="sm" disabled={busy} onClick={onTrust}>
          {t('accounts.certificate.trust', { defaultValue: 'Trust and continue' })}
        </Button>
      </div>
    </div>
  )
}
