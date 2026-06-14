import { useEffect, useState } from 'react'
import { Plus, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { setAccountAliases } from '../../states/accounts'
import type { Account, Alias } from '../../types'
import { SettingRow, SettingsGroup } from './AccountSettingsRows'

const fieldClass =
  'text-xs font-semibold bg-chats text-primary px-2.5 py-1 rounded-lg border border-border focus:outline-none focus:border-accent transition-colors'

// Send-as alias editor. Keeps all rows locally (including a half-typed one) and
// persists the list minus blank-email rows on every change.
export function AccountAliasesCard({ account }: { account: Account }) {
  const { t } = useTranslation()
  const [aliasesVal, setAliasesVal] = useState<Alias[]>([])

  // Seed when switching accounts only — not on every persist, so a half-typed
  // row isn't wiped by the normalized list coming back.
  useEffect(() => {
    setAliasesVal(account.aliases ?? [])
  }, [account.id])

  const updateAliases = (next: Alias[]) => {
    setAliasesVal(next)
    void setAccountAliases(
      account.id,
      next.filter((a) => a.email.trim()),
    )
  }

  return (
    <SettingsGroup title={t('settings.account.sending')}>
      <SettingRow
        title={t('settings.account.aliases')}
        hint={t('settings.account.aliasesHint')}
        control={
          <button
            onClick={() => updateAliases([...aliasesVal, { email: '', name: '' }])}
            className="flex items-center gap-1 text-xs font-semibold text-accent hover:underline cursor-pointer"
          >
            <Plus size={12} /> {t('settings.account.addAlias')}
          </button>
        }
      />
      {aliasesVal.map((alias, i) => (
        <div key={i} className="flex items-center gap-2 px-3.5 py-2">
          <input
            type="email"
            value={alias.email}
            onChange={(e) => updateAliases(aliasesVal.map((a, j) => (j === i ? { ...a, email: e.target.value } : a)))}
            className={`flex-1 min-w-0 ${fieldClass}`}
            placeholder="alias@example.com"
          />
          <input
            type="text"
            value={alias.name ?? ''}
            onChange={(e) => updateAliases(aliasesVal.map((a, j) => (j === i ? { ...a, name: e.target.value } : a)))}
            className={`w-28 shrink-0 ${fieldClass}`}
            placeholder={t('settings.account.aliasNamePlaceholder')}
          />
          <button
            onClick={() => updateAliases(aliasesVal.filter((_, j) => j !== i))}
            className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full hover:bg-active text-secondary transition-colors cursor-pointer"
            aria-label={t('settings.account.removeAlias')}
          >
            <Trash2 size={13} />
          </button>
        </div>
      ))}
    </SettingsGroup>
  )
}
