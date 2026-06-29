import { KeyRound, Mail, Rss } from 'lucide-react'
import type { ReactNode } from 'react'
import type { SetupMode } from '../../states/ui'

// Brand glyphs, sized by the caller. Shared by the OAuth sign-in rows.
export function GoogleIcon({ size = 20 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
      <path
        fill="#4285F4"
        d="M23.49 12.27c0-.79-.07-1.54-.19-2.27H12v4.51h6.47a5.54 5.54 0 0 1-2.4 3.64v2.98h3.88c2.27-2.1 3.54-5.2 3.54-8.86Z"
      />
      <path
        fill="#34A853"
        d="M12 24c3.24 0 5.96-1.07 7.95-2.87l-3.88-2.98c-1.07.72-2.44 1.14-4.07 1.14-3.13 0-5.78-2.11-6.73-4.95H1.26v3.07A12 12 0 0 0 12 24Z"
      />
      <path
        fill="#FBBC05"
        d="M5.27 14.34A7.2 7.2 0 0 1 4.9 12c0-.81.13-1.6.37-2.34V6.59H1.26A12 12 0 0 0 0 12c0 1.94.46 3.78 1.26 5.41l4.01-3.07Z"
      />
      <path
        fill="#EA4335"
        d="M12 4.71c1.76 0 3.34.61 4.59 1.8l3.44-3.44A11.55 11.55 0 0 0 12 0 12 12 0 0 0 1.26 6.59l4.01 3.07C6.22 6.82 8.87 4.71 12 4.71Z"
      />
    </svg>
  )
}

export function MicrosoftIcon({ size = 20 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 23 23" aria-hidden="true">
      <path fill="#f25022" d="M1 1h10v10H1z" />
      <path fill="#7fba00" d="M12 1h10v10H12z" />
      <path fill="#00a4ef" d="M1 12h10v10H1z" />
      <path fill="#ffb900" d="M12 12h10v10H12z" />
    </svg>
  )
}

export type ProviderDef = {
  id: 'oauth' | SetupMode
  label: string
  descriptionKey: string
  defaultDescription?: string
  mode: SetupMode
  isActive: (mode: SetupMode) => boolean
  icon: (size: number) => ReactNode
}

// The account setup tabs, in picker order. Gmail and Outlook share one visible
// OAuth tab; the OAuth panel lists concrete provider sign-in buttons.
export const PROVIDERS: ProviderDef[] = [
  {
    id: 'oauth',
    label: 'OAuth',
    descriptionKey: 'accounts.providers.oauthDescription',
    defaultDescription: 'Provider sign-in',
    mode: 'gmail',
    isActive: (mode) => mode === 'gmail' || mode === 'outlook',
    icon: (s) => <KeyRound size={s} className="text-accent" />,
  },
  {
    id: 'custom',
    label: 'IMAP / SMTP',
    descriptionKey: 'accounts.providers.customDescription',
    mode: 'custom',
    isActive: (mode) => mode === 'custom',
    icon: (s) => <Mail size={s} className="text-accent" />,
  },
  {
    id: 'rss',
    label: 'RSS / Atom',
    descriptionKey: 'accounts.providers.rssDescription',
    mode: 'rss',
    isActive: (mode) => mode === 'rss',
    icon: (s) => <Rss size={s} className="text-orange-500" />,
  },
]
