import { useValue } from '@legendapp/state/react'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { certTrust$, dismissCertificatePrompt, trustPromptedCertificate } from '../../states/certificateTrust'
import { CertificateTrustPanel } from './CertificateTrustPanel'

// The app-level certificate prompt: raised when an action on an existing
// account hits a server certificate we cannot validate — today, a send whose
// submission server was refused. Setup-time failures are handled inline by the
// account dialog instead, where the servers are already on screen.
export function CertificateTrustDialog() {
  const prompt = useValue(certTrust$.prompt)
  const busy = useValue(certTrust$.busy)

  useEscapeKey(() => {
    if (!busy) dismissCertificatePrompt()
  }, Boolean(prompt))

  if (!prompt) return null

  return (
    <div
      className="fixed inset-0 z-[120] flex items-center justify-center bg-black/45 p-4"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) dismissCertificatePrompt()
      }}
    >
      <section role="alertdialog" aria-modal="true" className="w-full max-w-md">
        <CertificateTrustPanel
          prompt={prompt}
          busy={busy}
          onTrust={trustPromptedCertificate}
          onDismiss={dismissCertificatePrompt}
        />
      </section>
    </div>
  )
}
