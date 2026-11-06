import { KeyRound } from 'lucide-react'
import type { KeyChange } from '../../lib/crypto'
import { useTranslation } from '../../lib/i18n'
import { crypto$ } from '../../states/crypto'

/**
 * The full editor's line above its footer when a recipient's key isn't the
 * one Meron last encrypted to for them: usually a new device or
 * app, but it could be someone else's key, so it offers Verify… for the new
 * key. The lock turns amber too, and Send asks once (`confirmKeyChanges`).
 */
export function KeyChangeNote({ accountId, changed }: { accountId: string; changed: KeyChange[] }) {
  const { t } = useTranslation()
  if (changed.length === 0) return null
  return (
    <div role="status" className="shrink-0 space-y-0.5 px-4 pb-1">
      {changed.map((change) => (
        <p key={change.addr} className="flex items-start gap-1.5 text-xs leading-snug text-secondary">
          <KeyRound size={13} className="mt-px shrink-0 text-amber-600 dark:text-amber-400" />
          <span className="select-text">
            {t('crypto.keyChange.note', { address: change.addr })}{' '}
            <button
              type="button"
              onClick={() =>
                crypto$.verify.set({ addr: change.addr, name: change.addr, fingerprint: change.current, accountId })
              }
              className="cursor-pointer font-semibold text-accent hover:underline"
            >
              {t('crypto.verifyKey')}
            </button>
          </span>
        </p>
      ))}
    </div>
  )
}
