import type { Account } from '../../types'
import { isRssAccount } from '../../lib/threadActions'
import { MessageContextMenu, type MessageContextMenuState } from '../chat/MessageContextMenu'

// Right-click menu for a starred-view row. RSS rows carry their full body and
// get the regular per-message menu; mail rows are headers only, so the actions
// that need a body (open in new tab / forward / edit as new) are hidden.
export function StarredItemMenu({
  state,
  accounts,
  onClose,
}: {
  state: MessageContextMenuState
  accounts: Account[]
  onClose: () => void
}) {
  const account = accounts.find((acc) => acc.id === state.message.account_id)
  const isRSS = isRssAccount(account, state.message.account_id)
  return (
    <MessageContextMenu
      state={isRSS ? state : { ...state, hideOpenInNewTab: true }}
      isRSS={isRSS}
      headerOnly={!isRSS}
      onClose={onClose}
    />
  )
}
