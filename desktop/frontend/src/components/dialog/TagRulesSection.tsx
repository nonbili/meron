import { useEffect, useState } from 'react'
import { Plus, Trash2 } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { loadTagRules, saveTagRules, type TagRule } from '../../states/mailTags'
import { IconButton } from '../button/IconButton'

const fieldClass =
  'min-w-0 flex-1 rounded-lg bg-hover px-2.5 py-1.5 text-xs text-primary placeholder-secondary border border-transparent focus:ring-1 focus:ring-accent'

// Tag new mail that matches a search. The search is the search box's own syntax,
// so a rule can be tried out there before it is saved here.
export function TagRulesSection() {
  const { t } = useTranslation()
  // Each save replaces the whole list, so nothing may start from a stale one:
  // not before the list has loaded, and not while another save is on its way.
  const [rules, setRules] = useState<TagRule[] | null>(null)
  const [saving, setSaving] = useState(false)
  const [query, setQuery] = useState('')
  const [tags, setTags] = useState('')
  const [invalid, setInvalid] = useState(false)

  useEffect(() => {
    void loadTagRules().then(setRules, () => setRules(null))
  }, [])

  const save = async (next: TagRule[]) => {
    if (saving) return
    setSaving(true)
    try {
      setRules(await saveTagRules(next))
      setQuery('')
      setTags('')
      setInvalid(false)
    } catch {
      setInvalid(true)
    } finally {
      setSaving(false)
    }
  }

  const add = () => {
    if (!rules) return
    const names = tags
      .split(',')
      .map((tag) => tag.trim())
      .filter(Boolean)
    void save([...rules, { id: '', query, tags: names }])
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto rounded-xl border border-border divide-y divide-border/40">
      {rules?.map((rule) => (
        <div key={rule.id} className="flex items-center gap-2 px-3.5 py-2 text-xs">
          <span className="min-w-0 flex-1 truncate font-medium text-primary">{rule.query}</span>
          <span className="max-w-40 truncate text-accent">{rule.tags.join(', ')}</span>
          <IconButton
            icon={Trash2}
            iconSize={13}
            label={t('tags.rules.delete')}
            size="sm"
            disabled={saving}
            onClick={() => void save(rules.filter((item) => item.id !== rule.id))}
          />
        </div>
      ))}
      <div className="flex items-center gap-2 px-3.5 py-2">
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('tags.rules.query')}
          aria-label={t('tags.rules.query')}
          className={fieldClass}
        />
        <input
          value={tags}
          onChange={(event) => setTags(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') add()
          }}
          placeholder={t('tags.rules.tags')}
          aria-label={t('tags.rules.tags')}
          className={fieldClass}
        />
        <IconButton
          icon={Plus}
          iconSize={14}
          label={t('tags.rules.add')}
          size="sm"
          disabled={saving || !rules}
          onClick={add}
        />
      </div>
      {invalid && <p className="px-3.5 py-2 text-[0.6875rem] font-medium text-rose-500">{t('tags.rules.invalid')}</p>}
    </div>
  )
}
