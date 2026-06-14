import { useEffect, useState } from 'react'
import { BellOff, Clock, Code, Eye, Image as ImageIcon, Inbox, Pause } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from 'react-i18next'
import {
  setAccountImages,
  setAccountConversationHtml,
  setAccountUnified,
  setAccountMuted,
  setAccountPaused,
  setRSSSyncInterval,
} from '../../states/accounts'
import { settings$, setAccountSidebarHidden } from '../../states/settings'
import type { Account } from '../../types'
import { NumberRow, SegmentedRow, SettingsGroup, ToggleRow } from './AccountSettingsRows'

// The grouped toggle sections of the account panel: visibility,
// notifications/sync (incl. the RSS interval), and content rendering.
export function AccountTogglesSection({ account, isRSS }: { account: Account; isRSS: boolean }) {
  const { t } = useTranslation()
  const hiddenSidebarAccounts = useValue(settings$.hiddenSidebarAccounts)
  const [rssIntervalVal, setRssIntervalVal] = useState('60')

  useEffect(() => {
    setRssIntervalVal(String(account.rss_sync_interval_minutes ?? 60))
  }, [account.id, account.rss_sync_interval_minutes])

  const inUnified = account.included_in_unified !== false
  const inSidebar = !hiddenSidebarAccounts.includes(account.id)
  const muted = account.muted ?? false
  const paused = account.paused ?? false
  const loadImages = account.load_remote_images ?? isRSS
  const conversationHtml = account.conversation_html ?? true

  const updateRSSInterval = (value: string) => {
    setRssIntervalVal(value)
    if (!value.trim()) return
    const minutes = Number(value)
    if (!Number.isFinite(minutes)) return
    void setRSSSyncInterval(account.id, minutes)
  }

  return (
    <>
      <SettingsGroup title={t('settings.account.visibility')}>
        <ToggleRow
          icon={<Inbox size={15} />}
          title={t('settings.account.showInUnifiedInbox')}
          hint={t('settings.account.showInUnifiedInboxHint')}
          checked={inUnified}
          onChange={() => setAccountUnified(account.id, !inUnified)}
        />
        <ToggleRow
          icon={<Eye size={15} />}
          title={t('settings.account.showInLeftSidebar')}
          hint={t('settings.account.showInLeftSidebarHint')}
          checked={inSidebar}
          onChange={() => setAccountSidebarHidden(account.id, inSidebar)}
        />
      </SettingsGroup>

      <SettingsGroup title={t('settings.account.notificationsSync')}>
        <ToggleRow
          icon={<BellOff size={15} />}
          title={t('settings.account.muteNotifications')}
          hint={t('settings.account.muteNotificationsHint')}
          checked={muted}
          onChange={() => setAccountMuted(account.id, !muted)}
        />
        <ToggleRow
          icon={<Pause size={15} />}
          title={t('settings.account.pauseAccount')}
          hint={t('settings.account.pauseAccountHint')}
          checked={paused}
          onChange={() => setAccountPaused(account.id, !paused)}
        />
        {isRSS && (
          <NumberRow
            icon={<Clock size={15} />}
            title={t('settings.account.syncInterval')}
            value={rssIntervalVal}
            min={5}
            max={1440}
            step={5}
            suffix="min"
            onChange={updateRSSInterval}
          />
        )}
      </SettingsGroup>

      <SettingsGroup title={t('settings.account.content')}>
        <ToggleRow
          icon={<ImageIcon size={15} />}
          title={t('settings.account.loadRemoteImages')}
          hint={t('settings.account.loadRemoteImagesHint')}
          checked={loadImages}
          onChange={() => setAccountImages(account.id, !loadImages)}
        />
        <SegmentedRow
          icon={<Code size={15} />}
          title={t('settings.account.conversationView')}
          value={conversationHtml ? 'html' : 'plain'}
          options={[
            { value: 'plain', label: t('settings.account.conversationPlain') },
            { value: 'html', label: 'HTML' },
          ]}
          onChange={(mode) => setAccountConversationHtml(account.id, mode === 'html')}
        />
      </SettingsGroup>
    </>
  )
}
