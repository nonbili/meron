import { Bell, BellOff, EyeOff, Pause, Play, SlidersHorizontal } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { setAccountMuted, setAccountPaused } from '../../states/accounts'
import { setAccountSidebarHidden } from '../../states/settings'
import { ui$ } from '../../states/ui'
import type { Account } from '../../types'
import { RailContextMenu, RailMenuItem } from './RailContextMenu'

const secondary = 'text-secondary'

// Right-click menu for an account avatar: settings, mute, pause, hide.
export function AccountContextMenu({
  account,
  x,
  y,
  onClose,
}: {
  account: Account
  x: number
  y: number
  onClose: () => void
}) {
  const { t } = useTranslation()
  return (
    <RailContextMenu x={x} y={y} onClose={onClose}>
      <RailMenuItem
        icon={<SlidersHorizontal size={13} className={secondary} />}
        label={t('settings.account.accountSettings')}
        onClick={() => {
          ui$.accountSettingsId.set(account.id)
          ui$.settingsOpen.set(true)
          onClose()
        }}
      />
      <RailMenuItem
        icon={account.muted ? <Bell size={13} className={secondary} /> : <BellOff size={13} className={secondary} />}
        label={account.muted ? t('settings.account.unmuteNotifications') : t('settings.account.muteNotifications')}
        onClick={() => {
          void setAccountMuted(account.id, !(account.muted ?? false))
          onClose()
        }}
      />
      <RailMenuItem
        icon={account.paused ? <Play size={13} className={secondary} /> : <Pause size={13} className={secondary} />}
        label={account.paused ? t('settings.account.resumeChecking') : t('settings.account.pauseAccount')}
        onClick={() => {
          void setAccountPaused(account.id, !(account.paused ?? false))
          onClose()
        }}
      />
      <RailMenuItem
        icon={<EyeOff size={13} className={secondary} />}
        label={t('sidebar.actions.hideFromSidebar')}
        onClick={() => {
          setAccountSidebarHidden(account.id, true)
          onClose()
        }}
      />
    </RailContextMenu>
  )
}
