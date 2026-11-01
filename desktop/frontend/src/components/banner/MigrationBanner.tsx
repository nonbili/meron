import { Loader2 } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { connectivity$ } from '../../states/connectivity'

// Thin banner shown while the core upgrades its store on the first start after
// an update. Nothing can load until that finishes, and it has no progress to
// report, so this only says what the wait is for.
export function MigrationBanner() {
  const { t } = useTranslation()
  const migrating = useValue(connectivity$.migrating)

  if (!migrating) return null

  return (
    <div
      role="status"
      className="flex shrink-0 items-center justify-center gap-2 border-b border-accent/20 bg-accent/10 px-3 py-1.5 text-xs font-medium text-primary"
    >
      <Loader2 size={13} className="shrink-0 animate-spin" />
      <span className="min-w-0 truncate">{t('connectivity.upgradingStore')}</span>
    </div>
  )
}
