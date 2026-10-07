import { useEffect, useState } from 'react'
import { BellOff, Clock, Code, Eye, Image as ImageIcon, Inbox, Pause, Send } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import {
  setAccountImages,
  setAccountConversationHtml,
  setAccountUnified,
  setAccountMuted,
  setAccountPaused,
  setAccountSaveSentCopy,
  setRSSSyncInterval,
} from '../../states/accounts'
import { mail$ } from '../../states/mail'
import { ensureAccountFolders, isNotifiableFolder, setFolderNotify } from '../../states/mailFolders'
import { settings$, setAccountSideNavHidden } from '../../states/settings'
import type { Account } from '../../types'
import { FolderSwitcher } from '../menu/FolderSwitcher'
import { NumberRow, SegmentedRow, SettingRow, SettingsGroup, ToggleRow } from './AccountSettingsRows'

function sentCopyDefault(account: Account): boolean {
  const host = account.smtp_host.trim().replace(/\.$/, '').toLowerCase()
  const providerSavesSent =
    account.auth_type === 'gmail_oauth' ||
    account.auth_type === 'outlook_oauth' ||
    [
      'smtp.gmail.com',
      'smtp.googlemail.com',
      'smtp-mail.outlook.com',
      'smtp.office365.com',
      'smtp.live.com',
      'smtp.hotmail.com',
    ].includes(host)
  return !providerSavesSent
}

// Which of a mail account's folders notify about new mail: the inbox always
// does, the rest are opted in one by one. Only the opted-in ones are listed, so
// the group stays short however many folders the account has; the rest are a
// pick away. The same switch as the folder's own menu, gathered in one place.
function FolderNotificationsGroup({ accountId }: { accountId: string }) {
  const { t } = useTranslation()
  const folders = useValue(mail$.foldersByAccount[accountId]) ?? []
  // Folders switched off here keep their row until the panel closes, so a
  // mis-click can be undone where it was made.
  const [kept, setKept] = useState<string[]>([])

  useEffect(() => {
    setKept([])
    void ensureAccountFolders(accountId, { refreshIfBootstrapOnly: true })
  }, [accountId])

  const notifiable = folders.filter(isNotifiableFolder)
  const listed = notifiable.filter((folder) => folder.notify || kept.includes(folder.id))
  const addLabel = t('settings.account.folderNotificationsAdd')

  return (
    <SettingsGroup title={t('settings.account.folderNotifications')}>
      <SettingRow
        title={t('folders.roles.inbox')}
        hint={t('settings.account.folderNotificationsHint')}
        control={
          <span className="text-xs font-normal text-secondary">{t('settings.account.folderNotificationsAlways')}</span>
        }
      />
      {listed.map((folder) => (
        <ToggleRow
          key={folder.id}
          title={folder.name || folder.id}
          checked={!!folder.notify}
          onChange={() => {
            if (folder.notify) setKept((ids) => (ids.includes(folder.id) ? ids : [...ids, folder.id]))
            void setFolderNotify(accountId, folder.id, !folder.notify, folder.name)
          }}
        />
      ))}
      {/* Always offered: the picker loads the folder list and reports on it itself. */}
      <div className="flex min-h-11 items-center px-1.5 py-2">
        <FolderSwitcher
          accountId={accountId}
          folderId=""
          label={addLabel}
          title={addLabel}
          labelClassName="text-xs font-normal text-accent"
          // Offer only what turning on would change: not the inbox and the
          // other folders that can't notify, nor the ones already on.
          takenFolderIds={folders.filter((folder) => !isNotifiableFolder(folder) || folder.notify).map((f) => f.id)}
          onSelect={(folderId) => {
            const folder = folders.find((item) => item.id === folderId)
            void setFolderNotify(accountId, folderId, true, folder?.name)
          }}
        />
      </div>
    </SettingsGroup>
  )
}

// The grouped toggle sections of the account panel: visibility,
// notifications/sync (incl. the RSS interval), and content rendering.
export function AccountTogglesSection({ account, isRSS }: { account: Account; isRSS: boolean }) {
  const { t } = useTranslation()
  const hiddenSideNavAccounts = useValue(settings$.hiddenSideNavAccounts)
  const [rssIntervalVal, setRssIntervalVal] = useState('60')

  useEffect(() => {
    setRssIntervalVal(String(account.rss_sync_interval_minutes ?? 60))
  }, [account.id, account.rss_sync_interval_minutes])

  const inUnified = account.included_in_unified !== false
  const inSideNav = !hiddenSideNavAccounts.includes(account.id)
  const muted = account.muted ?? false
  const paused = account.paused ?? false
  const loadImages = account.load_remote_images ?? isRSS
  const conversationHtml = account.conversation_html ?? true
  const saveSentCopy = account.save_sent_copy ?? sentCopyDefault(account)

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
          title={t('settings.account.showInSideNav')}
          hint={t('settings.account.showInSideNavHint')}
          checked={inSideNav}
          onChange={() => setAccountSideNavHidden(account.id, inSideNav)}
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

      {!isRSS && <FolderNotificationsGroup accountId={account.id} />}

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
        {!isRSS && (
          <ToggleRow
            icon={<Send size={15} />}
            title={t('settings.account.saveSentCopies')}
            hint={t('settings.account.saveSentCopiesHint')}
            checked={saveSentCopy}
            onChange={() => setAccountSaveSentCopy(account.id, !saveSentCopy)}
          />
        )}
      </SettingsGroup>
    </>
  )
}
