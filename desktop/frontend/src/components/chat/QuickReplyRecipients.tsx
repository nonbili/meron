import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openReplyInFullEditor, quickReplyRecipients } from '../../states/compose'
import { parseAddressList, type AddressItem } from './messageHelpers'

// How many names to spell out before collapsing the rest into "+N".
const VISIBLE_NAMES = 2

function summarize(items: AddressItem[]): string {
  if (items.length <= VISIBLE_NAMES) return items.map((item) => item.name).join(', ')
  const shown = items.slice(0, VISIBLE_NAMES).map((item) => item.name)
  return `${shown.join(', ')} +${items.length - VISIBLE_NAMES}`
}

// Who the quick reply goes to. The box has no address fields, and a plain reply
// keeps the Cc while dropping the other To recipients — a recipient set worth
// disclosing rather than leaving to be discovered in the Sent copy. Read-only:
// clicking opens the full editor, where the addresses can actually be edited.
export function QuickReplyRecipients() {
  const { t } = useTranslation()
  const { to, cc } = useValue(quickReplyRecipients)
  const toItems = parseAddressList(to)
  const ccItems = parseAddressList(cc)

  if (toItems.length === 0) return null

  const full = [to, cc].filter(Boolean).join(', ')

  return (
    <button
      onClick={() => openReplyInFullEditor()}
      title={`${t('composer.actions.openFullEditor')} — ${full}`}
      className="flex min-w-0 shrink items-center gap-1 rounded-lg px-1.5 py-0.5 text-[0.6875rem] text-secondary hover:bg-active hover:text-primary transition-colors cursor-pointer"
    >
      <span className="font-semibold shrink-0">{t('composer.fields.to')}</span>
      <span className="truncate">{summarize(toItems)}</span>
      {ccItems.length > 0 && (
        <>
          <span className="font-semibold shrink-0">{t('composer.fields.cc')}</span>
          <span className="truncate">{summarize(ccItems)}</span>
        </>
      )}
    </button>
  )
}
