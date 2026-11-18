import { useTranslation } from '../../lib/i18n'
import { setAccountColor } from '../../states/accounts'
import type { Account } from '../../types'
import { ColorSwatchPicker } from '../field/ColorSwatchPicker'
import { SettingRow } from './AccountSettingsRows'

// The colour that marks this account's rows in lists that mix accounts. Saved
// once chosen, not while the system picker is dragged: each save is a write.
export function AccountColorRow({ account }: { account: Account }) {
  const { t } = useTranslation()

  return (
    <SettingRow
      title={t('settings.account.color')}
      hint={t('settings.account.colorHint')}
      control={
        <ColorSwatchPicker
          value={account.color ?? null}
          onChange={(color) => void setAccountColor(account.id, color)}
          label={t('settings.account.color')}
          noneLabel={t('settings.account.colorNone')}
        />
      }
    />
  )
}
