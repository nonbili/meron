import { useEffect, useState } from 'react'
import { Pencil, Plus, Trash2 } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { loadRules, saveRules, type Rule, type RuleMove } from '../../states/mailRules'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { TextInput } from '../field/Field'
import { SelectRow, SettingsGroup, ToggleRow } from './AccountSettingsRows'

type Draft = { query: string; tags: string; mark_read: boolean; star: boolean; move_to: RuleMove; folder: string }

const EMPTY_DRAFT: Draft = { query: '', tags: '', mark_read: false, star: false, move_to: '', folder: '' }

// What to do with new mail that matches a search. The search is the search box's
// own syntax, so a rule can be tried out there before it is saved here.
export function RulesPanel() {
  const { t } = useTranslation()
  // Each save replaces the whole list, so nothing may start from a stale one:
  // not before the list has loaded, and not while another save is on its way.
  const [rules, setRules] = useState<Rule[] | null>(null)
  const [saving, setSaving] = useState(false)
  const [draft, setDraft] = useState<Draft>(EMPTY_DRAFT)
  const [editing, setEditing] = useState<string | null>(null)
  const [invalid, setInvalid] = useState(false)

  useEffect(() => {
    void loadRules().then(setRules, () => setRules(null))
  }, [])

  const change = (patch: Partial<Draft>) => {
    setDraft({ ...draft, ...patch })
    setInvalid(false)
  }

  const reset = () => {
    setDraft(EMPTY_DRAFT)
    setEditing(null)
    setInvalid(false)
  }

  const save = async (next: Rule[]) => {
    if (saving) return false
    setSaving(true)
    try {
      setRules(await saveRules(next))
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
    const tags = draft.tags
      .split(',')
      .map((tag) => tag.trim())
      .filter(Boolean)
    const rule: Rule = { ...draft, id: editing ?? '', tags }
    const next = editing ? rules.map((item) => (item.id === editing ? rule : item)) : [...rules, rule]
    if (await save(next)) reset()
  }

  const edit = (rule: Rule) => {
    setDraft({ ...rule, tags: rule.tags.join(', ') })
    setEditing(rule.id)
    setInvalid(false)
  }

  const remove = async (rule: Rule) => {
    if (!rules) return
    if ((await save(rules.filter((item) => item.id !== rule.id))) && editing === rule.id) reset()
  }

  const summary = (rule: Rule) =>
    [
      ...rule.tags,
      rule.mark_read && t('notification.actionMarkRead'),
      rule.star && t('chat.star'),
      rule.move_to === 'archive' && t('folders.roles.archive'),
      rule.move_to === 'trash' && t('folders.roles.trash'),
      rule.move_to === 'folder' && rule.folder,
    ]
      .filter(Boolean)
      .join(' · ')

  return (
    <div className="flex flex-col gap-4">
      <p className="text-[0.75rem] text-secondary">{t('rules.hint')}</p>

      <div className="rounded-xl border border-border divide-y divide-border/40">
        {rules?.length === 0 && <p className="px-3.5 py-3 text-xs text-secondary">{t('rules.empty')}</p>}
        {rules?.map((rule) => (
          <div key={rule.id} className="flex items-center gap-2 px-3.5 py-2 text-xs">
            <div className="min-w-0 flex-1">
              <p className="truncate font-medium text-primary">{rule.query}</p>
              <p className="truncate text-accent">{summary(rule)}</p>
            </div>
            <IconButton
              icon={Pencil}
              iconSize={13}
              label={t('rules.edit')}
              size="sm"
              disabled={saving}
              onClick={() => edit(rule)}
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
        ))}
      </div>

      <SettingsGroup title={editing ? t('rules.edit') : t('rules.add')}>
        <div className="flex flex-col gap-2 px-3.5 py-2.5">
          <TextInput
            value={draft.query}
            onChange={(event) => change({ query: event.target.value })}
            placeholder={t('rules.query')}
            aria-label={t('rules.query')}
            className="w-full"
          />
          <TextInput
            value={draft.tags}
            onChange={(event) => change({ tags: event.target.value })}
            placeholder={t('rules.tags')}
            aria-label={t('rules.tags')}
            className="w-full"
          />
        </div>
        <ToggleRow
          title={t('notification.actionMarkRead')}
          checked={draft.mark_read}
          onChange={() => change({ mark_read: !draft.mark_read })}
        />
        <ToggleRow title={t('chat.star')} checked={draft.star} onChange={() => change({ star: !draft.star })} />
        <SelectRow
          title={t('rules.moveTo')}
          value={draft.move_to}
          options={[
            { value: '', label: t('rules.moveNone') },
            { value: 'archive', label: t('folders.roles.archive') },
            { value: 'trash', label: t('folders.roles.trash') },
            { value: 'folder', label: t('rules.moveFolder') },
          ]}
          onChange={(value) => change({ move_to: value as RuleMove })}
        />
        {draft.move_to === 'folder' && (
          <div className="px-3.5 py-2.5">
            <TextInput
              value={draft.folder}
              onChange={(event) => change({ folder: event.target.value })}
              placeholder={t('rules.folder')}
              aria-label={t('rules.folder')}
              className="w-full"
            />
          </div>
        )}
        {draft.move_to === 'trash' && (
          <p className="px-3.5 py-2.5 text-[0.6875rem] font-medium text-rose-500">{t('rules.trashHint')}</p>
        )}
        {invalid && <p className="px-3.5 py-2.5 text-[0.6875rem] font-medium text-rose-500">{t('rules.invalid')}</p>}
        <div className="flex justify-end gap-2 px-3.5 py-2.5">
          {editing && (
            <Button variant="secondary" onClick={reset}>
              {t('buttons.cancel')}
            </Button>
          )}
          <Button leftIcon={editing ? undefined : Plus} disabled={saving || !rules} onClick={() => void submit()}>
            {editing ? t('buttons.save') : t('rules.add')}
          </Button>
        </div>
      </SettingsGroup>
    </div>
  )
}
