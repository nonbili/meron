import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { DndContext, closestCenter } from '@dnd-kit/core'
import { restrictToVerticalAxis } from '@dnd-kit/modifiers'
import { SortableContext, arrayMove, useSortable, verticalListSortingStrategy } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { useValue } from '@legendapp/state/react'
import { GripVertical, Pencil, Play, Plus, Square, Trash2 } from 'lucide-react'
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
import { EMPTY_FIELDS, fieldsEmpty, type SearchFields } from '../search/searchFields'
import { inputClass, Row } from '../search/SearchOptions'
import { TagInput } from '../search/TagInput'
import { useTaskDragSensors } from '../tasks/taskDrag'
import { accountLabel } from '../threads/ThreadMoveCopyItems'
import { SegmentedRow, SettingRow, SettingsGroup, Switch, ToggleRow } from './AccountSettingsRows'
import { RuleFolderPicker, useRuleTargetLabel } from './RuleFolderPicker'

type Draft = {
  fields: SearchFields
  trigger: RuleTrigger
  delay_seconds: number
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

const RUN_POLL_MS = 1500
const ACTIVITY_SHOWN = 20

const EMPTY_DRAFT: Draft = {
  fields: EMPTY_FIELDS,
  trigger: 'arrival',
  delay_seconds: 0,
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
  editing: boolean
  onMove: (to: number) => void
  children: ReactNode
}

// Dragged by its handle, since the rest of the row is controls. The arrow keys
// move it instead of dnd-kit's keyboard sensor, which would lose Escape to the
// dialog around it.
function RuleRow({ id, at, locked, editing, onMove, children }: RuleRowProps) {
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
      className={`flex items-center gap-2 px-3.5 py-2 text-xs ${
        isDragging ? 'relative z-10 bg-raised' : editing ? 'bg-active/60' : ''
      }`}
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
    const { fields, ...actions } = draft
    const saved = await save(async () => {
      const rule: Rule = {
        ...actions,
        id: editing ?? '',
        query: await ruleQuery(fields),
        tags,
        accounts: chosenAccounts,
        // The list's switch stays live while the form is open.
        disabled: rules.find((item) => item.id === editing)?.disabled ?? false,
      }
      return editing ? rules.map((item) => (item.id === editing ? rule : item)) : [...rules, rule]
    })
    if (saved) reset()
  }

  const edit = async (rule: Rule) => {
    const request = ++formRequest.current
    const { query: _, id: __, ...actions } = rule
    const fields = await ruleFields(rule.query)
    if (request !== formRequest.current) return
    setDraft({
      ...actions,
      trigger: rule.trigger ?? 'arrival',
      delay_seconds: rule.delay_seconds ?? 0,
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

  const summary = (rule: Rule) =>
    [
      (rule.trigger && rule.trigger !== 'arrival') || rule.delay_seconds
        ? `${t(`rules.trigger.${rule.trigger ?? 'arrival'}`)} · ${t('rules.delaySummary', { minutes: (rule.delay_seconds ?? 0) / 60 })}`
        : '',
      rule.tags.length > 0 && t('rules.summaryTags', { tags: rule.tags.join(', ') }),
      rule.mark_read && t('notification.actionMarkRead'),
      rule.star && t('chat.star'),
      rule.copy_folder && t('rules.summaryCopy', { folder: copyLabel(rule) }),
      rule.move_to && t('rules.summaryMove', { folder: moveLabel(rule) }),
    ]
      .filter(Boolean)
      .join(' · ')

  const formOpen = adding || editing !== null

  return (
    <div className="flex flex-col gap-4">
      <p className="text-[0.75rem] text-secondary">{t('rules.timingHint')}</p>

      {rules?.length === 0 && !formOpen && <p className="text-xs text-secondary">{t('rules.empty')}</p>}
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
                  <RuleRow
                    key={rule.id}
                    id={rule.id}
                    at={at}
                    locked={saving}
                    editing={editing === rule.id}
                    onMove={(to) => void move(at, to)}
                  >
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
                        disabled={saving || starting !== null || editing === rule.id || rule.disabled}
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

      {!formOpen && (
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
      )}

      {formOpen && (
        <>
          {/* The address suggestions open below their field, past the card when it is the last. */}
          <SettingsGroup title={t('rules.when')} overflowVisible>
            <Row label={t('rules.trigger')}>
              <select
                aria-label={t('rules.trigger')}
                className={inputClass}
                value={draft.trigger}
                onChange={(event) => change({ trigger: event.target.value as RuleTrigger })}
              >
                {(['arrival', 'read', 'age'] as const).map((trigger) => (
                  <option key={trigger} value={trigger}>
                    {t(`rules.trigger.${trigger}`)}
                  </option>
                ))}
              </select>
            </Row>
            <Row label={t('rules.delayMinutes')}>
              <input
                aria-label={t('rules.delayMinutes')}
                type="number"
                min={0}
                max={5256000}
                step="any"
                className={inputClass}
                value={draft.delay_seconds / 60}
                onChange={(event) => change({ delay_seconds: Math.round(Number(event.target.value) * 60) })}
              />
            </Row>
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
                  options={[
                    { label: t('rules.copyNone'), onPick: () => change({ copy_account: '', copy_folder: '' }) },
                  ]}
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
              disabled={
                saving ||
                !rules ||
                (scoped && chosenAccounts.length === 0) ||
                !Number.isFinite(draft.delay_seconds) ||
                draft.delay_seconds < 0 ||
                draft.delay_seconds > 315360000
              }
              onClick={() => void submit()}
            >
              {editing ? t('buttons.save') : t('rules.add')}
            </Button>
          </div>
        </>
      )}

      {!formOpen && status.log.length > 0 && (
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
