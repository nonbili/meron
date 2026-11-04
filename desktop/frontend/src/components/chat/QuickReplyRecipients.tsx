import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { openReplyInFullEditor } from '../../states/compose'
import { quickReplyAddable, quickReplyRecipients } from '../../states/composeReply'
import { addToQuickReply } from '../../states/quickReply'
import { MenuItem } from '../menu/MenuItem'
import { MenuSurface } from '../menu/MenuSurface'
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
// When Reply all would reach more people, hovering the line (or focusing it)
// lists them, each as the Cc it would become; a click copies that one person on
// this reply, without leaving the box, and the line then names them in its Cc.
export function QuickReplyRecipients() {
  const { t } = useTranslation()
  const { to, cc } = useValue(quickReplyRecipients)
  const addable = useValue(quickReplyAddable)
  const toItems = parseAddressList(to)
  const ccItems = parseAddressList(cc)

  if (toItems.length === 0) return null

  const full = [to, cc].filter(Boolean).join(', ')

  return (
    // The list sits above the line, joined to it by its bottom padding so the
    // pointer can travel from one to the other without the hover ending.
    <span className="group relative flex min-w-0 shrink items-center">
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
      {addable.length > 0 && (
        <div className="absolute bottom-full right-0 z-50 hidden pb-1 group-hover:block group-focus-within:block">
          <MenuSurface className="min-w-[160px] max-w-[min(320px,80vw)]">
            <ul aria-label={t('chat.actions.replyAll')}>
              {addable.map((entry) => (
                <li key={entry} title={entry}>
                  <MenuItem
                    icon={<span className="shrink-0 font-semibold text-secondary">{t('composer.fields.cc')}</span>}
                    label={<span className="truncate">{parseAddressList(entry)[0]?.name || entry}</span>}
                    onClick={() => addToQuickReply(entry)}
                  />
                </li>
              ))}
            </ul>
          </MenuSurface>
        </div>
      )}
    </span>
  )
}
