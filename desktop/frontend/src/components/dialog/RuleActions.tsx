import { useRef, useState } from 'react'
import { Plus, X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { isLocalAccountId } from '../../states/accounts'
import type { RuleMove } from '../../states/mailRules'
import { RecipientInput } from '../composer/RecipientInput'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import { inputClass } from '../search/SearchOptions'
import { TagInput } from '../search/TagInput'
import { flyoutClass } from '../threads/ThreadMoveCopyItems'
import { RuleFolderPicker } from './RuleFolderPicker'

export type RuleAction = 'tags' | 'mark_read' | 'star' | 'copy' | 'forward' | 'move'

/** Every action, in the order a rule's rows show them. */
const RULE_ACTIONS: RuleAction[] = ['tags', 'mark_read', 'star', 'copy', 'forward', 'move']

/** What a rule for sent mail can't do: it leaves the copy in Sent, read, and sends nothing. */
const NOT_FOR_SENT: RuleAction[] = ['mark_read', 'forward', 'move']

export type ActionValues = {
  tags: string
  mark_read: boolean
  star: boolean
  copy_folder: string
  copy_account: string
  forward_to: string
  move_to: RuleMove
  folder: string
  folder_account: string
}

const LABELS: Record<RuleAction, string> = {
  tags: 'rules.addTags',
  mark_read: 'notification.actionMarkRead',
  star: 'chat.star',
  copy: 'rules.copyTo',
  forward: 'rules.forwardTo',
  move: 'rules.moveTo',
}

/** What removing an action's row leaves its values at. */
const CLEARED: Record<RuleAction, Partial<ActionValues>> = {
  tags: { tags: '' },
  mark_read: { mark_read: false },
  star: { star: false },
  copy: { copy_folder: '', copy_account: '' },
  forward: { forward_to: '' },
  move: { move_to: '', folder: '', folder_account: '' },
}

/** The actions a rule takes, which are the rows its editor opens with. */
export function ruleActions(values: ActionValues): RuleAction[] {
  const set: Record<RuleAction, boolean> = {
    tags: values.tags.trim() !== '',
    mark_read: values.mark_read,
    star: values.star,
    copy: values.copy_folder !== '',
    forward: values.forward_to.trim() !== '',
    move: values.move_to !== '',
  }
  return RULE_ACTIONS.filter((action) => set[action])
}

/** The rows a rule for sent mail keeps. */
export const sentActions = (shown: RuleAction[]) => shown.filter((action) => !NOT_FOR_SENT.includes(action))

/**
 * One row per action the rule takes, as conditions have, so a rule shows what
 * it does rather than every action it could take. A row added but left without
 * a value is no action at all.
 */
export function RuleActions({
  values,
  shown,
  sent,
  trashHint,
  copyLabel,
  moveLabel,
  onChange,
}: {
  values: ActionValues
  shown: RuleAction[]
  sent: boolean
  /** What the warning under a move to Trash says, which depends on the conditions. */
  trashHint: string
  copyLabel: (values: ActionValues) => string
  moveLabel: (values: ActionValues) => string
  onChange: (patch: Partial<ActionValues>, shown: RuleAction[]) => void
}) {
  const { t } = useTranslation()
  // The row just asked for, which takes the focus or opens its picker.
  const [added, setAdded] = useState<RuleAction | null>(null)
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  const addRef = useRef<HTMLButtonElement | null>(null)
  const available = RULE_ACTIONS.filter((action) => !(sent && NOT_FOR_SENT.includes(action)))
  const rows = available.filter((action) => shown.includes(action))
  const addable = available.filter((action) => !shown.includes(action))

  const add = (action: RuleAction) => {
    setMenu(null)
    setAdded(action)
    // Mark as read and Star have no value: the row is the action.
    const patch = action === 'mark_read' ? { mark_read: true } : action === 'star' ? { star: true } : {}
    onChange(patch, [...shown, action])
  }

  const remove = (action: RuleAction) =>
    onChange(
      CLEARED[action],
      shown.filter((item) => item !== action),
    )

  const control = (action: RuleAction) => {
    const label = t(LABELS[action])
    const autoFocus = added === action
    switch (action) {
      case 'tags':
        return <TagInput value={values.tags} autoFocus={autoFocus} onChange={(tags) => onChange({ tags }, shown)} />
      case 'copy':
        return (
          <RuleFolderPicker
            label={label}
            value={values.copy_folder ? copyLabel(values) : t('rules.chooseFolder')}
            options={[]}
            autoOpen={autoFocus}
            onPick={({ accountId, folderId }) => onChange({ copy_account: accountId, copy_folder: folderId }, shown)}
          />
        )
      case 'forward':
        return (
          <RecipientInput
            bare
            autoFocus={autoFocus}
            value={values.forward_to}
            onChange={(forward_to) => onChange({ forward_to }, shown)}
            accountId=""
            className={`${inputClass} w-full`}
          />
        )
      case 'move': {
        const pick = (move_to: RuleMove, folder = '', folder_account = '') =>
          onChange({ move_to, folder, folder_account }, shown)
        return (
          <RuleFolderPicker
            label={label}
            value={values.move_to ? moveLabel(values) : t('rules.chooseFolder')}
            options={[
              { label: t('folders.roles.archive'), onPick: () => pick('archive') },
              { label: t('folders.roles.trash'), onPick: () => pick('trash') },
            ]}
            autoOpen={autoFocus}
            onPick={({ accountId, folderId }) => pick('folder', folderId, accountId)}
          />
        )
      }
      default:
        return null
    }
  }

  return (
    <>
      {rows.map((action) => {
        // A field takes the room it has; a picker sits at the end of its row.
        const wide = action === 'tags' || action === 'forward'
        return (
          <div key={action} data-action={action}>
            <div className="flex min-h-11 items-center gap-4 px-3.5 py-2">
              <span className="shrink-0 text-xs font-normal text-primary">{t(LABELS[action])}</span>
              {!wide && <span className="flex-1" />}
              {control(action)}
              <button
                type="button"
                aria-label={`${t('rules.removeAction')}: ${t(LABELS[action])}`}
                title={t('rules.removeAction')}
                onClick={() => remove(action)}
                className="-mr-1 flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded-md text-secondary transition-colors hover:bg-active hover:text-primary"
              >
                <X size={13} />
              </button>
            </div>
            {action === 'forward' && values.forward_to.trim() && (
              <p className="px-3.5 pb-2.5 text-[0.6875rem] text-secondary">{t('rules.forwardHint')}</p>
            )}
            {action === 'move' && values.move_to === 'trash' && (
              <p className="px-3.5 pb-2.5 text-[0.6875rem] font-medium text-rose-500">{trashHint}</p>
            )}
            {action === 'move' && values.move_to === 'folder' && isLocalAccountId(values.folder_account) && (
              <p className="px-3.5 pb-2.5 text-[0.6875rem] text-secondary">{t('rules.localMoveHint')}</p>
            )}
          </div>
        )
      })}
      {addable.length > 0 && (
        <div className="px-3.5 py-2.5">
          <button
            ref={addRef}
            type="button"
            aria-expanded={menu !== null}
            onClick={() => {
              if (menu) return setMenu(null)
              const rect = addRef.current?.getBoundingClientRect()
              setMenu({ x: rect?.left ?? 0, y: (rect?.bottom ?? 0) + 4 })
            }}
            className="flex w-fit cursor-pointer items-center gap-1.5 rounded-md py-1 text-xs font-semibold text-accent hover:underline"
          >
            <Plus size={13} />
            {t('rules.addAction')}
          </button>
          {menu && (
            <FloatingContextMenu
              x={menu.x}
              y={menu.y}
              onClose={() => setMenu(null)}
              overlay
              overlayClassName="fixed inset-0 z-[61]"
              className={`${flyoutClass} min-w-[180px]`}
            >
              {addable.map((action) => (
                <MenuItem key={action} label={t(LABELS[action])} onClick={() => add(action)} />
              ))}
            </FloatingContextMenu>
          )}
        </div>
      )}
    </>
  )
}
