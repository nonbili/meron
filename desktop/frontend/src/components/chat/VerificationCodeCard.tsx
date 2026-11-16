import { Check, Copy, KeyRound } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from '../../lib/i18n'
import { copyText } from '../../lib/native'

// The one-time code of a "here is your sign-in code" mail, above the message
// body: one click copies it. The core decides which mails carry one. Mirrors
// mobile's VerificationCodeCard.
export function VerificationCodeCard({ code }: { code: string }) {
  const { t } = useTranslation()
  const [copied, setCopied] = useState(false)
  const reset = useRef<ReturnType<typeof setTimeout>>(undefined)
  useEffect(() => () => clearTimeout(reset.current), [])

  const copy = () => {
    copyText(code)
      .then(() => {
        setCopied(true)
        clearTimeout(reset.current)
        reset.current = setTimeout(() => setCopied(false), 2000)
      })
      .catch(() => undefined)
  }

  return (
    <button
      type="button"
      onClick={copy}
      className="mb-2 flex max-w-full items-center gap-2 rounded-xl border border-border/20 bg-black/[0.03] px-3 py-2 text-xs dark:bg-white/[0.03]"
    >
      <KeyRound size={15} className="shrink-0 text-accent" />
      <span className="font-mono text-sm font-semibold tracking-widest select-text">{code}</span>
      <span className="flex items-center gap-1 text-secondary">
        {copied ? <Check size={12} className="shrink-0" /> : <Copy size={12} className="shrink-0" />}
        {copied ? t('notification.codeCopied') : t('notification.actionCopyCode')}
      </span>
    </button>
  )
}
