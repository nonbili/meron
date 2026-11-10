import { PenLine, PenOff } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { accounts$ } from '../../states/accounts'
import { compose$ } from '../../states/composeState'
import { quickReplySignature, setQuickReplySignatureOff } from '../../states/quickReply'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'

/**
 * The quick reply's signature switch. The signature isn't in the box; it is
 * added below the reply when it is sent, and this shows that it will be (the
 * tooltip holds the signature itself). A click leaves it off for this reply
 * only. Not shown when the replying account sends no signature.
 *
 * Not lucide's `Signature` icon: CryptoBadge uses it for OpenPGP-signed mail,
 * and beside the lock it would read as "this reply is signed with my key".
 */
export function QuickReplySignatureToggle() {
  const { t } = useTranslation()
  // quickReplySignature() peeks, so subscribe to what it reads.
  useValue(ui$.selectedThread)
  useValue(ui$.selectedAccount)
  useValue(accounts$)
  useValue(settings$.signature)
  const off = useValue(compose$.quickReplySignatureOff)
  const signature = quickReplySignature().text
  if (!signature) return null

  const label = off ? t('composer.signature.off') : t('composer.signature.on')
  return (
    <button
      type="button"
      role="switch"
      aria-checked={!off}
      aria-label={label}
      aria-description={signature}
      title={`${label}\n\n${signature}`}
      onClick={() => setQuickReplySignatureOff(!off)}
      className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-secondary hover:bg-active hover:text-primary transition-colors cursor-pointer"
    >
      {off ? <PenOff size={15} /> : <PenLine size={15} />}
    </button>
  )
}
