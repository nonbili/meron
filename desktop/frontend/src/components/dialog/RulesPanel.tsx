import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { DndContext, closestCenter } from '@dnd-kit/core'
import { restrictToVerticalAxis } from '@dnd-kit/modifiers'
import { SortableContext, arrayMove, useSortable, verticalListSortingStrategy } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { useValue } from '@legendapp/state/react'
import { ArrowLeft, GripVertical, Pencil, Play, Plus, Square, Trash2 } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { accounts$, isSendableAccount } from '../../states/accounts'
import {
  loadRules,
  loadRuleStatus,
  ruleFields,
  ruleMatches,
  ruleQuery,
  runRule,
  saveRules,
  stopRule,
  type Rule,
  type RuleMove,
  type RuleTrigger,
  type RuleStatus,
} from '../../states/mailRules'
import { confirmAction, showToast } from '../../states/ui'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { AccountAvatar } from '../avatar/AccountAvatar'
import { RecipientInput } from '../composer/RecipientInput'
import { Checkbox } from '../field/Checkbox'
import { SelectInput, TextInput } from '../field/Field'
import { EMPTY_FIELDS, fieldsEmpty, type SearchFields } from '../search/searchFields'
import { inputClass, Row } from '../search/SearchOptions'
import { TagInput } from '../search/TagInput'
import { useTaskDragSensors } from '../tasks/taskDrag'
import { accountLabel } from '../threads/ThreadMoveCopyItems'
import { SegmentedRow, SettingRow, SettingsGroup, Switch, ToggleRow } from './AccountSettingsRows'
import { RuleFolderPicker, useRuleTargetLabel } from './RuleFolderPicker'

type Timing = 'now' | RuleTrigger
type Unit = 'minutes' | 'hours' | 'days'

const TIMINGS: Timing[] = ['now', 'arrival', 'read', 'age']
const UNITS: Unit[] = ['minutes', 'hours', 'days']
const UNIT_SECONDS: Record<Unit, number> = { minutes: 60, hours: 3600, days: 86400 }
const MAX_DELAY_SECONDS = 315360000

// The largest unit that holds the delay whole, so 30 days isn't shown as 43200 minutes.
function splitDelay(seconds: number): { amount: number; unit: Unit } {
  const unit =
    (['days', 'hours'] as const).find((unit) => seconds > 0 && seconds % UNIT_SECONDS[unit] === 0) ?? 'minutes'
  return { amount: seconds / UNIT_SECONDS[unit], unit }
}

type Draft = {
  fields: SearchFields
  // "Right away" is arrival with no delay, kept apart so that choosing "After
  // it arrives" doesn't fall back to it while the amount is still empty.
  timing: Timing
  amount: string
  unit: Unit
  tags: string
  mark_read: boolean
  star: boolean
  move_to: RuleMove
  folder: string
  folder_account: string
  accounts: string[]
  copy_folder: string
  copy_account: string
  disabled: boolean
}

/** The draft's delay in seconds, or null while the amount isn't a usable one. */
function draftDelay({ timing, amount, unit }: Draft) {
  if (timing === 'now') return 0
  const value = Number(amount)
  if (!amount.trim() || !Number.isFinite(value) || value < 0) return null
  const seconds = Math.round(value * UNIT_SECONDS[unit])
  return seconds <= MAX_DELAY_SECONDS ? seconds : null
}

const RUN_POLL_MS = 1500
const ACTIVITY_SHOWN = 20

const EMPTY_DRAFT: Draft = {
  fields: EMPTY_FIELDS,
  timing: 'now',
  amount: '',
  unit: 'minutes',
  tags: '',
  mark_read: false,
  star: false,
  move_to: '',
  folder: '',
  folder_account: '',
  accounts: [],
  copy_folder: '',
  copy_account: '',
  disabled: false,
}

type RuleRowProps = {
  id: string
  at: number
  locked: boolean
  onMove: (to: number) => void
  children: ReactNode
}

// Dragged by its handle, since the rest of the row is controls. The arrow keys
// move it instead of dnd-kit's keyboard sensor, which would lose Escape to the
// dialog around it.
function RuleRow({ id, at, locked, onMove, children }: RuleRowProps) {
  const { t } = useTranslation()
  const { listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({
    id,
    disabled: locked,
  })
  const handle = useRef<HTMLButtonElement | null>(null)
  const keepFocus = useRef(false)

  // Moving a row in the DOM can drop the focus it holds.
  useLayoutEffect(() => {
    if (keepFocus.current) handle.current?.focus()
    keepFocus.current = false
  }, [at])

  return (
    <div
      ref={setNodeRef}
      data-rule={id}
      style={{ transform: CSS.Translate.toString(transform), transition: isDragging ? 'none' : transition }}
      className={`flex items-center gap-2 px-3.5 py-2 text-xs ${isDragging ? 'relative z-10 bg-raised' : ''}`}
    >
      <button
        ref={(node) => {
          handle.current = node
          setActivatorNodeRef(node)
        }}
        type="button"
        title={t('rules.reorder')}
        aria-label={t('rules.reorder')}
        aria-keyshortcuts="ArrowUp ArrowDown"
        aria-disabled={locked}
        className="-ml-1.5 flex h-6 w-4 shrink-0 cursor-grab touch-none items-center justify-center rounded text-secondary hover:text-primary active:cursor-grabbing"
        {...listeners}
        onKeyDown={(event) => {
          if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
          event.preventDefault()
          keepFocus.current = true
          onMove(at + (event.key === 'ArrowUp' ? -1 : 1))
        }}
      >
        <GripVertical size={13} />
      </button>
      {children}
    </div>
  )
}

// What to do with new mail that matches some conditions, filled in as fields. A
// rule stores them as search text, which the form never shows; conditions it has
// no field for stay in that text across an edit.
export function RulesPanel() {
  const { t } = useTranslation()
  const allAccounts = useValue(accounts$)
  const mailAccounts = allAccounts.filter(isSendableAccount)
  const targetLabel = useRuleTargetLabel()
  // Each save replaces the whole list, so nothing may start from a stale one:
  // not before the list has loaded, and not while another save is on its way.
  const [rules, setRules] = useState<Rule[] | null>(null)
  const [saving, setSaving] = useState(false)
  const [draft, setDraft] = useState<Draft>(EMPTY_DRAFT)
  const [editing, setEditing] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)
  // Apart from `draft.accounts`: picking "selected" starts with none ticked,
  // which as a list would mean all of them.
  const [scoped, setScoped] = useState(false)
  const [invalid, setInvalid] = useState(false)
  // Opening a rule waits on the core: only the last thing asked for may land.
  const formRequest = useRef(0)
  const [starting, setStarting] = useState<string | null>(null)
  const [status, setStatus] = useState<RuleStatus>({ log: [], running: {} })
  const [conditions, setConditions] = useState<Record<string, SearchFields>>({})
  const sensors = useTaskDragSensors()

  useEffect(() => {
    void loadRules().then(setRules, () => setRules(null))
  }, [])

  useEffect(() => {
    let current = true
    void Promise.all((rules ?? []).map(async (rule) => [rule.query, await ruleFields(rule.query)] as const)).then(
      (entries) => {
        if (current) setConditions(Object.fromEntries(entries))
      },
    )
    return () => {
      current = false
    }
  }, [rules])

  const refreshStatus = () => void loadRuleStatus().then(setStatus, () => {})

  useEffect(refreshStatus, [])

  // Only while a run has work left: the log otherwise changes on its own terms,
  // and this page isn't where anyone waits for new mail.
  const runActive = Object.keys(status.running).length > 0
  useEffect(() => {
    if (!runActive) return
    const timer = setInterval(refreshStatus, RUN_POLL_MS)
    return () => clearInterval(timer)
  }, [runActive])

  const change = (patch: Partial<Draft>) => {
    setDraft({ ...draft, ...patch })
    setInvalid(false)
  }

  // Always asks: it can touch far more mail than a rule normally does, and a
  // save may have changed what the rule does since it last ran.
  const run = async (rule: Rule) => {
    setStarting(rule.id)
    try {
      const count = await ruleMatches(rule.id)
      if (count === 0) {
        showToast(t('rules.runNothing'))
        return
      }
      const removes = rule.move_to === 'trash' || (rule.move_to === 'folder' && rule.folder_account !== '')
      const confirmed = await confirmAction({
        title: t('rules.run'),
        message: t(rule.move_to === 'trash' ? 'rules.runTrashConfirm' : 'rules.runConfirm', { count }),
        confirmLabel: t('rules.runAction'),
        cancelLabel: t('buttons.cancel'),
        tone: removes ? 'danger' : 'default',
      })
      if (!confirmed) return
      showToast(t('rules.runStarted', { count: await runRule(rule.id) }))
    } catch {
      showToast(t('rules.runFailed'), 'error')
    } finally {
      setStarting(null)
      refreshStatus()
    }
  }

  const stop = async (rule: Rule) => {
    await stopRule(rule.id).catch(() => showToast(t('rules.runFailed'), 'error'))
    refreshStatus()
  }

  const known = (id: string) => allAccounts.some((account) => account.id === id)

  // A rule keeps a removed account until edited, so that it says what is wrong
  // instead of quietly becoming a different rule.
  const lostAccount = (rule: Rule) =>
    [...rule.accounts, rule.copy_account, rule.folder_account].some((id) => id && !known(id))

  const lastFailure = (rule: Rule) => {
    const last = status.log.find((entry) => entry.rule_id === rule.id)
    return last?.failed ? last : undefined
  }

  const setDisabled = (rule: Rule, disabled: boolean) =>
    rules && save(() => rules.map((item) => (item.id === rule.id ? { ...item, disabled } : item)))

  // Order matters: the first rule to move a message decides where it goes. The
  // new order shows at once, or a dropped row would jump back until the save.
  const move = async (from: number, to: number) => {
    if (!rules || saving || to < 0 || to >= rules.length || from === to) return
    const next = arrayMove(rules, from, to)
    setRules(next)
    if (!(await save(() => next))) setRules(rules)
  }

  const chosenAccounts = draft.accounts.filter(known)

  const pickMove = (move_to: RuleMove, folder = '', folder_account = '') => change({ move_to, folder, folder_account })

  const changeFields = (patch: Partial<SearchFields>) => change({ fields: { ...draft.fields, ...patch } })

  const reset = () => {
    formRequest.current += 1
    setDraft(EMPTY_DRAFT)
    setEditing(null)
    setAdding(false)
    setScoped(false)
    setInvalid(false)
  }

  const save = async (next: () => Rule[] | Promise<Rule[]>) => {
    if (saving) return false
    setSaving(true)
    try {
      setRules(await saveRules(await next()))
      return true
    } catch {
      setInvalid(true)
      return false
    } finally {
      setSaving(false)
    }
  }

  const submit = async () => {
    if (!rules) return
    const tags = draft.tags.split(/[\s,]+/).filter(Boolean)
    const { fields, timing, amount: _, unit: __, ...actions } = draft
    const delay = draftDelay(draft)
    if (delay === null) return
    const saved = await save(async () => {
      const rule: Rule = {
        ...actions,
        trigger: timing === 'now' ? 'arrival' : timing,
        delay_seconds: delay,
        id: editing ?? '',
        query: await ruleQuery(fields),
        tags,
        accounts: chosenAccounts,
        // Editing doesn't turn a rule on or off.
        disabled: rules.find((item) => item.id === editing)?.disabled ?? false,
      }
      return editing ? rules.map((item) => (item.id === editing ? rule : item)) : [...rules, rule]
    })
    if (saved) reset()
  }

  const edit = async (rule: Rule) => {
    const request = ++formRequest.current
    const { query: _, id: __, trigger = 'arrival', delay_seconds: delay = 0, ...actions } = rule
    const fields = await ruleFields(rule.query)
    if (request !== formRequest.current) return
    const timing = trigger === 'arrival' && delay === 0 ? 'now' : trigger
    const { amount, unit } = splitDelay(delay)
    setDraft({
      ...actions,
      timing,
      amount: timing === 'now' ? '' : String(amount),
      unit,
      fields,
      tags: rule.tags.join(' '),
    })
    setEditing(rule.id)
    setAdding(false)
    setScoped(rule.accounts.length > 0)
    setInvalid(false)
  }

  const remove = async (rule: Rule) => {
    if (!rules) return
    const confirmed = await confirmAction({
      title: t('rules.delete'),
      message: `${condition(rule)}\n${t('rules.deleteConfirm')}`,
      confirmLabel: t('buttons.delete'),
      cancelLabel: t('buttons.cancel'),
      tone: 'danger',
    })
    if (!confirmed) return
    if ((await save(() => rules.filter((item) => item.id !== rule.id))) && editing === rule.id) reset()
  }

  // The rule's conditions in the form's own words, not as search text.
  const condition = (rule: Rule) => {
    const fields = conditions[rule.query]
    if (!fields) return rule.query || t('rules.allMail')
    const valued = (label: string, value: string) => value.trim() && `${label}: ${value.trim()}`
    return [
      fieldsEmpty(fields) && t('rules.allMail'),
      valued(t('composer.fields.from'), fields.from),
      valued(t('composer.fields.to'), fields.to),
      valued(t('composer.fields.subject'), fields.subject),
      valued(t('tags.title'), fields.tags),
      valued(t('search.hasWords'), fields.words),
      valued(t('search.after'), fields.after),
      valued(t('search.before'), fields.before),
      fields.unread && t('filters.unread'),
      fields.starred && t('filters.starred'),
      fields.has_attachment && t('filters.hasAttachment'),
    ]
      .filter(Boolean)
      .join(' · ')
  }

  const moveLabel = ({ move_to, folder, folder_account }: Draft | Rule) => {
    if (move_to === 'archive') return t('folders.roles.archive')
    if (move_to === 'trash') return t('folders.roles.trash')
    return folder_account ? targetLabel({ accountId: folder_account, folderId: folder }) : folder
  }

  const copyLabel = ({ copy_account, copy_folder }: Draft | Rule) =>
    targetLabel({ accountId: copy_account, folderId: copy_folder })

  const timingSummary = ({ trigger = 'arrival', delay_seconds: delay = 0 }: Rule) => {
    if (trigger === 'arrival' && delay === 0) return ''
    const { amount, unit } = splitDelay(delay)
    return t(`rules.summary.${trigger}`, { duration: t(`rules.duration.${unit}`, { count: amount }) })
  }

  // A delayed choice starts from a typical amount rather than an empty, unsavable one.
  const pickTiming = (timing: Timing) => {
    if (timing === 'now') change({ timing, amount: '' })
    else if (draft.amount) change({ timing })
    else change({ timing, ...(timing === 'age' ? { amount: '30', unit: 'days' } : { amount: '5', unit: 'minutes' }) })
  }

  const summary = (rule: Rule) =>
    [
      timingSummary(rule),
      rule.tags.length > 0 && t('rules.summaryTags', { tags: rule.tags.join(', ') }),
      rule.mark_read && t('notification.actionMarkRead'),
      rule.star && t('chat.star'),
      rule.copy_folder && t('rules.summaryCopy', { folder: copyLabel(rule) }),
      rule.move_to && t('rules.summaryMove', { folder: moveLabel(rule) }),
    ]
      .filter(Boolean)
      .join(' · ')

  const formOpen = adding || editing !== null
  const delay = draftDelay(draft)

  // Adding or editing takes over the panel, as an MCP client does: the list
  // stays an overview, and the form isn't read as part of it.
  if (formOpen) {
    return (
      <div className="flex flex-col gap-4">
        <div className="flex items-center gap-2">
          <button
            type="button"
            aria-label={t('buttons.back')}
            title={t('buttons.back')}
            onClick={reset}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-secondary transition-colors hover:bg-active hover:text-primary cursor-pointer"
          >
            <ArrowLeft size={16} />
          </button>
          <h2 className="truncate text-base font-bold leading-tight tracking-tight">
            {editing ? t('rules.edit') : t('rules.add')}
          </h2>
        </div>
        {/* The address suggestions open below their field, past the card when it is the last. */}
        <SettingsGroup title={t('rules.when')} overflowVisible>
          {(mailAccounts.length > 1 || scoped) && (
            <>
              <SegmentedRow
                title={t('rules.scope')}
                value={scoped ? 'selected' : 'all'}
                options={[
                  { value: 'all', label: t('kanban.allAccounts') },
                  { value: 'selected', label: t('rules.selectedAccounts') },
                ]}
                onChange={(value) => {
                  setScoped(value === 'selected')
                  if (value === 'all') change({ accounts: [] })
                }}
              />
              {scoped && (
                <div className="flex flex-col gap-2 px-3.5 py-2.5">
                  {mailAccounts.map((account) => (
                    <label
                      key={account.id}
                      className="flex cursor-pointer items-center gap-2 text-xs text-primary select-none"
                    >
                      <Checkbox
                        checked={draft.accounts.includes(account.id)}
                        onChange={(event) =>
                          change({
                            accounts: event.target.checked
                              ? [...draft.accounts, account.id]
                              : draft.accounts.filter((id) => id !== account.id),
                          })
                        }
                      />
                      <AccountAvatar
                        accountId={account.id}
                        name={accountLabel(account)}
                        src={account.avatar_url}
                        size={18}
                        className="shrink-0"
                      />
                      <span className="min-w-0 truncate">{accountLabel(account)}</span>
                    </label>
                  ))}
                </div>
              )}
            </>
          )}
          <div className="flex flex-col gap-2 px-3.5 py-2.5">
            <Row label={t('composer.fields.from')}>
              <RecipientInput
                bare
                value={draft.fields.from}
                onChange={(from) => changeFields({ from })}
                accountId=""
                className={`${inputClass} w-full`}
              />
            </Row>
            <Row label={t('composer.fields.to')}>
              <RecipientInput
                bare
                value={draft.fields.to}
                onChange={(to) => changeFields({ to })}
                accountId=""
                className={`${inputClass} w-full`}
              />
            </Row>
            <Row label={t('composer.fields.subject')}>
              <input
                value={draft.fields.subject}
                onChange={(event) => changeFields({ subject: event.target.value })}
                className={inputClass}
              />
            </Row>
            <Row label={t('search.hasWords')}>
              <input
                value={draft.fields.words}
                onChange={(event) => changeFields({ words: event.target.value })}
                className={inputClass}
              />
            </Row>
            <label className="flex cursor-pointer items-center gap-2 pt-1 text-xs text-primary select-none">
              <Checkbox
                checked={draft.fields.has_attachment}
                onChange={(event) => changeFields({ has_attachment: event.target.checked })}
              />
              {t('filters.hasAttachment')}
            </label>
            <p className="text-[0.6875rem] text-secondary">{t('rules.queryHint')}</p>
          </div>
        </SettingsGroup>

        <SettingsGroup title={t('rules.timing')}>
          <SettingRow
            title={t('rules.runWhen')}
            control={
              <SelectInput
                aria-label={t('rules.timing')}
                value={draft.timing}
                onChange={(event) => pickTiming(event.target.value as Timing)}
                className="w-48 rounded-xl py-1.5 pl-3 font-semibold"
              >
                {TIMINGS.map((timing) => (
                  <option key={timing} value={timing} className="bg-chats text-primary">
                    {t(`rules.timing.${timing}`)}
                  </option>
                ))}
              </SelectInput>
            }
          />
          {draft.timing !== 'now' && (
            <SettingRow
              title={t(draft.timing === 'age' ? 'rules.age' : 'rules.wait')}
              control={
                <div className="flex items-center gap-2">
                  <TextInput
                    aria-label={t(draft.timing === 'age' ? 'rules.age' : 'rules.wait')}
                    type="number"
                    min={0}
                    step="any"
                    value={draft.amount}
                    invalid={delay === null}
                    aria-invalid={delay === null}
                    onChange={(event) => change({ amount: event.target.value })}
                    className="w-20 font-semibold"
                  />
                  <SelectInput
                    aria-label={t('rules.unit')}
                    value={draft.unit}
                    onChange={(event) => change({ unit: event.target.value as Unit })}
                    className="w-28 rounded-xl py-1.5 pl-3 font-semibold"
                  >
                    {UNITS.map((unit) => (
                      <option key={unit} value={unit} className="bg-chats text-primary">
                        {t(`rules.unit.${unit}`)}
                      </option>
                    ))}
                  </SelectInput>
                </div>
              }
            />
          )}
          <p className="px-3.5 py-2.5 text-[0.6875rem] text-secondary">{t('rules.timingHint')}</p>
        </SettingsGroup>

        {/* The tag suggestions open below their row, past the card when it is the last. */}
        <SettingsGroup title={t('rules.then')} overflowVisible>
          <div className="flex min-h-11 items-center gap-4 px-3.5 py-2">
            <span className="shrink-0 text-xs font-normal text-primary">{t('rules.addTags')}</span>
            <TagInput value={draft.tags} onChange={(tags) => change({ tags })} />
          </div>
          <ToggleRow
            title={t('notification.actionMarkRead')}
            checked={draft.mark_read}
            onChange={() => change({ mark_read: !draft.mark_read })}
          />
          <ToggleRow title={t('chat.star')} checked={draft.star} onChange={() => change({ star: !draft.star })} />
          <SettingRow
            title={t('rules.copyTo')}
            control={
              <RuleFolderPicker
                label={t('rules.copyTo')}
                value={draft.copy_folder ? copyLabel(draft) : t('rules.copyNone')}
                options={[{ label: t('rules.copyNone'), onPick: () => change({ copy_account: '', copy_folder: '' }) }]}
                onPick={({ accountId, folderId }) => change({ copy_account: accountId, copy_folder: folderId })}
              />
            }
          />
          <SettingRow
            title={t('rules.moveTo')}
            control={
              <RuleFolderPicker
                label={t('rules.moveTo')}
                value={draft.move_to ? moveLabel(draft) : t('rules.moveNone')}
                options={[
                  { label: t('rules.moveNone'), onPick: () => pickMove('') },
                  { label: t('folders.roles.archive'), onPick: () => pickMove('archive') },
                  { label: t('folders.roles.trash'), onPick: () => pickMove('trash') },
                ]}
                onPick={({ accountId, folderId }) => pickMove('folder', folderId, accountId)}
              />
            }
          />
          {draft.move_to === 'trash' && (
            <p className="px-3.5 py-2.5 text-[0.6875rem] font-medium text-rose-500">
              {t(fieldsEmpty(draft.fields) ? 'rules.trashAllHint' : 'rules.trashHint')}
            </p>
          )}
        </SettingsGroup>

        {invalid && <p className="text-[0.6875rem] font-medium text-rose-500">{t('rules.invalid')}</p>}
        <div className="flex justify-end gap-2">
          <Button variant="secondary" onClick={reset}>
            {t('buttons.cancel')}
          </Button>
          <Button
            leftIcon={editing ? undefined : Plus}
            disabled={saving || !rules || (scoped && chosenAccounts.length === 0) || delay === null}
            onClick={() => void submit()}
          >
            {editing ? t('buttons.save') : t('rules.add')}
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-4">
      {rules?.length === 0 && <p className="text-xs text-secondary">{t('rules.empty')}</p>}
      {rules && rules.length > 0 && (
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          modifiers={[restrictToVerticalAxis]}
          onDragEnd={({ active, over }) => {
            if (!over) return
            const at = (id: unknown) => rules.findIndex((rule) => rule.id === id)
            void move(at(active.id), at(over.id))
          }}
        >
          <SortableContext items={rules.map((rule) => rule.id)} strategy={verticalListSortingStrategy}>
            <div className="overflow-hidden rounded-xl border border-border divide-y divide-border/40">
              {rules.map((rule, at) => {
                const left = status.running[rule.id]
                const failure = lastFailure(rule)
                const lost = lostAccount(rule)
                return (
                  <RuleRow key={rule.id} id={rule.id} at={at} locked={saving} onMove={(to) => void move(at, to)}>
                    <Switch
                      checked={!rule.disabled}
                      label={t('rules.enabled')}
                      onChange={() => {
                        if (!(rule.disabled && lost)) void setDisabled(rule, !rule.disabled)
                      }}
                    />
                    <div className="min-w-0 flex-1">
                      <div className={`flex min-w-0 items-center gap-1.5 ${rule.disabled ? 'opacity-50' : ''}`}>
                        {rule.accounts.map((id) => {
                          const account = allAccounts.find((item) => item.id === id)
                          const name = account ? accountLabel(account) : id
                          return (
                            <span key={id} title={name} className="shrink-0">
                              <AccountAvatar accountId={id} name={name} src={account?.avatar_url} size={16} />
                            </span>
                          )
                        })}
                        <p className="min-w-0 truncate font-medium text-primary">{condition(rule)}</p>
                      </div>
                      <p className={`truncate text-accent ${rule.disabled ? 'opacity-50' : ''}`}>{summary(rule)}</p>
                      {left !== undefined && (
                        <p className="truncate text-secondary">{t('rules.runLeft', { count: left })}</p>
                      )}
                      {lost && <p className="text-rose-500">{t('rules.accountGone')}</p>}
                      {failure && (
                        <p className="truncate text-rose-500" title={failure.detail}>
                          {t('rules.lastFailed')}
                        </p>
                      )}
                    </div>
                    {left !== undefined ? (
                      <IconButton
                        icon={Square}
                        iconSize={13}
                        label={t('rules.stop')}
                        size="sm"
                        onClick={() => void stop(rule)}
                      />
                    ) : (
                      <IconButton
                        icon={Play}
                        iconSize={13}
                        label={t('rules.run')}
                        size="sm"
                        disabled={saving || starting !== null || rule.disabled}
                        onClick={() => void run(rule)}
                      />
                    )}
                    <IconButton
                      icon={Pencil}
                      iconSize={13}
                      label={t('rules.edit')}
                      size="sm"
                      disabled={saving}
                      onClick={() => void edit(rule)}
                    />
                    <IconButton
                      icon={Trash2}
                      iconSize={13}
                      label={t('rules.delete')}
                      size="sm"
                      disabled={saving}
                      onClick={() => void remove(rule)}
                    />
                  </RuleRow>
                )
              })}
            </div>
          </SortableContext>
        </DndContext>
      )}

      <div>
        <Button
          leftIcon={Plus}
          disabled={!rules}
          onClick={() => {
            formRequest.current += 1
            setAdding(true)
          }}
        >
          {t('rules.add')}
        </Button>
      </div>

      {status.log.length > 0 && (
        <SettingsGroup title={t('rules.activity')}>
          {status.log.slice(0, ACTIVITY_SHOWN).map((entry, at) => (
            <div key={at} className="flex items-baseline justify-between gap-3 px-3.5 py-2 text-xs">
              <span
                className={`min-w-0 truncate ${entry.failed ? 'text-rose-500' : 'text-primary'}`}
                title={entry.detail || undefined}
              >
                {entry.subject || t('rules.activityRun', { count: entry.count })}
              </span>
              <span className="shrink-0 text-secondary">{new Date(entry.at * 1000).toLocaleString()}</span>
            </div>
          ))}
        </SettingsGroup>
      )}
    </div>
  )
}
