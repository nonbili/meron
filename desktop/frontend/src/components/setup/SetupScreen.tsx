import { invoke } from '../../lib/bridge'
import { useTranslation } from '../../lib/i18n'
import { showToast } from '../../states/ui'
import { AccountDialog } from '../dialog/AccountDialog'

export function SetupScreen() {
  const { t } = useTranslation()
  return (
    <main className="flex h-full w-full flex-col items-center overflow-y-auto bg-app px-6 py-10 text-primary">
      <button
        type="button"
        className="mb-4 shrink-0 cursor-pointer rounded-lg px-4 py-2 text-sm text-secondary hover:bg-hover hover:text-primary"
        onClick={() => void invoke('eml.open').catch((error) => showToast(String(error), 'error'))}
      >
        {t('eml.open')}
      </button>
      <AccountDialog variant="setup" />
    </main>
  )
}
