import { useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { ArrowDown, ArrowUp, Pencil, Play, Plus, Square, Trash2 } from 'lucide-react'
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
import { accountLabel } from '../threads/ThreadMoveCopyItems'
import { SegmentedRow, SettingRow, SettingsGroup, Switch, ToggleRow } from './AccountSettingsRows'
import { RuleFolderPicker, useRuleTargetLabel } from './RuleFolderPicker'

type Draft = {
  fields: SearchFields
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

  // Order matters: the first rule to move a message decides where it goes.
  const shift = (at: number, by: number) => {
    if (!rules) return
    const next = [...rules]
    const [rule] = next.splice(at, 1)
    next.splice(at + by, 0, rule)
    void save(() => next)
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
    setDraft({ ...actions, fields, tags: rule.tags.join(' ') })
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
      <p className="text-[0.75rem] text-secondary">{t('rules.hint')}</p>

      {rules?.length === 0 && !formOpen && <p className="text-xs text-secondary">{t('rules.empty')}</p>}
      {rules && rules.length > 0 && (
        <div className="overflow-hidden rounded-xl border border-border divide-y divide-border/40">
          {rules.map((rule, at) => {
            const left = status.running[rule.id]
            const failure = lastFailure(rule)
            const lost = lostAccount(rule)
            return (
              <div
                key={rule.id}
                className={`flex items-center gap-2 px-3.5 py-2 text-xs ${editing === rule.id ? 'bg-active/60' : ''}`}
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
                <IconButton
                  icon={ArrowUp}
                  iconSize={13}
                  label={t('rules.moveUp')}
                  size="sm"
                  disabled={saving || at === 0}
                  onClick={() => shift(at, -1)}
                />
                <IconButton
                  icon={ArrowDown}
                  iconSize={13}
                  label={t('rules.moveDown')}
                  size="sm"
                  disabled={saving || at === rules.length - 1}
                  onClick={() => shift(at, 1)}
                />
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
              </div>
            )
          })}
        </div>
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
              disabled={saving || !rules || (scoped && chosenAccounts.length === 0)}
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
