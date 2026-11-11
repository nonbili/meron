import { useValue } from '@legendapp/state/react'
import { accounts$ } from '../../states/accounts'
import { compose$ } from '../../states/composeState'
import { quickReplySignature, setQuickReplySignatureOff } from '../../states/quickReply'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import { SignatureToggle } from '../composer/SignatureToggle'

/**
 * The quick reply's signature switch. The signature isn't in the box; it is
 * added below the reply when it is sent, and this shows that it will be. A
 * click leaves it off for this reply only. Disabled while the box holds a
 * saved draft's own, edited or older copy, which switching on would add the
 * current one under. Not shown when the replying account sends no signature.
 */
export function QuickReplySignatureToggle() {
  // quickReplySignature() peeks, so subscribe to what it reads.
  useValue(ui$.selectedThread)
  useValue(ui$.selectedAccount)
  useValue(accounts$)
  useValue(settings$.signature)
  const off = useValue(compose$.quickReplySignatureOff)
  const unmanaged = useValue(compose$.quickReplySignatureUnmanaged)
  const signature = quickReplySignature().text
  if (!signature) return null
  return (
    <SignatureToggle
      size="md"
      state={unmanaged ? 'unmanaged' : off ? 'off' : 'on'}
      signature={signature}
      onChange={(on) => setQuickReplySignatureOff(!on)}
    />
  )
}
