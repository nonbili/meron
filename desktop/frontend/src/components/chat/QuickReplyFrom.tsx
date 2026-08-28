import { useState } from 'react'
import { Check, ChevronDown } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { compose$, quickReplyFromState, saveQuickReplyDraft } from '../../states/compose'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'

// Menu geometry, used to open the picker *upward* — the quick reply sits at the
// bottom of the window, so a downward menu would cover the composer it belongs
// to. FloatingContextMenu clamps to the viewport, so an estimate is enough.
const ITEM_HEIGHT_PX = 32
const MENU_PADDING_PX = 8
const MENU_GAP_PX = 6

// The quick reply's send-as identity: shows which address the reply goes out
// from, and opens a picker to override it. Renders nothing unless the account
// actually has more than one identity to choose between — with a single
// address there is nothing to disclose.
export function QuickReplyFrom() {
  const { t } = useTranslation()
  const { identities, selected } = useValue(quickReplyFromState)
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)

  if (identities.length === 0 || !selected) return null

  const label = selected.name ? `${selected.name} <${selected.email}>` : selected.email
  const menuHeight = identities.length * ITEM_HEIGHT_PX + MENU_PADDING_PX

  return (
    <>
      <button
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect()
          setMenu({ x: rect.left, y: rect.top - menuHeight - MENU_GAP_PX })
        }}
        title={t('composer.actions.chooseSendAddress')}
        className="flex max-w-full items-center gap-1 self-start rounded-lg px-1.5 py-0.5 text-[0.6875rem] text-secondary hover:bg-active hover:text-primary transition-colors cursor-pointer"
      >
        <span className="font-semibold">{t('composer.fields.from')}</span>
        <span className="truncate">{label}</span>
        <ChevronDown size={11} className="shrink-0" />
      </button>

      {menu && (
        <FloatingContextMenu
          x={menu.x}
          y={menu.y}
          overlay
          onClose={() => setMenu(null)}
          className="fixed z-50 min-w-[220px] max-w-[min(420px,90vw)] rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.preventDefault()}
        >
          {identities.map((identity) => (
            <MenuItem
              key={identity.email}
              icon={
                identity.email === selected.email ? (
                  <Check size={13} className="shrink-0 text-accent" />
                ) : (
                  <span className="w-[13px] shrink-0" />
                )
              }
              label={
                <span className="truncate">
                  {identity.name ? `${identity.name} <${identity.email}>` : identity.email}
                </span>
              }
              onClick={() => {
                setMenu(null)
                compose$.quickReplyFrom.set(identity.email)
                // The saved draft carries the From header, so re-save it against
                // the newly chosen identity rather than waiting for the next
                // keystroke's debounce.
                if (compose$.quickReplyDraftSaved.peek()) void saveQuickReplyDraft()
              }}
            />
          ))}
        </FloatingContextMenu>
      )}
    </>
  )
}
