import { useEffect, useState } from 'react'
import { useTranslation } from '../../lib/i18n'
import { setAccountName, setAccountSenderName } from '../../states/accounts'
import type { Account } from '../../types'
import { SettingsGroup, TextRow } from './AccountSettingsRows'

// Meron-only display name and outgoing sender name. The avatar itself is edited
// by clicking the header avatar above this group. Owns its own form state,
// seeded from the account and persisted on change.
export function AccountProfileGroup({ account, isRSS }: { account: Account; isRSS: boolean }) {
  const { t } = useTranslation()
  const [displayNameVal, setDisplayNameVal] = useState('')
  const [senderNameVal, setSenderNameVal] = useState('')

  useEffect(() => {
    setDisplayNameVal(account.display_name || '')
    setSenderNameVal(account.sender_name || '')
  }, [account.id, account.display_name, account.sender_name])

  return (
    <SettingsGroup title={t('settings.account.profile')}>
      <TextRow
        title={t('settings.account.displayName')}
        hint={t('settings.account.displayNameHint')}
        value={displayNameVal}
        placeholder={t('settings.account.displayNamePlaceholder')}
        onChange={(value) => {
          setDisplayNameVal(value)
          void setAccountName(account.id, value)
        }}
      />
      {!isRSS && (
        <TextRow
          title={t('settings.account.senderName')}
          hint={t('settings.account.senderNameHint')}
          value={senderNameVal}
          placeholder={t('settings.account.senderNamePlaceholder')}
          onChange={(value) => {
            setSenderNameVal(value)
            void setAccountSenderName(account.id, value)
          }}
        />
      )}
    </SettingsGroup>
  )
}
