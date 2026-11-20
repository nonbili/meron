import { AlertTriangle } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { sendsPlainText } from '../../lib/plainHttp'

/** Under a DAV server form: a plain `http://` address to another machine
 * sends the password unencrypted. Shown while typing; nothing is blocked. */
export function PlainHttpWarning({ server, mailPassword }: { server: string; mailPassword: boolean }) {
  const { t } = useTranslation()
  if (!sendsPlainText(server)) return null
  return (
    <p role="note" className="flex gap-1.5 text-[0.6875rem] leading-snug text-amber-600 dark:text-amber-400">
      <AlertTriangle size={13} aria-hidden className="mt-px shrink-0" />
      <span>
        {mailPassword
          ? t('accounts.plainHttpMailWarning', {
              defaultValue:
                'Not encrypted: your mail account’s password is sent as plain text. Use only on a network you trust.',
            })
          : t('accounts.plainHttpWarning', {
              defaultValue: 'Not encrypted: the password is sent as plain text. Use only on a network you trust.',
            })}
      </span>
    </p>
  )
}
