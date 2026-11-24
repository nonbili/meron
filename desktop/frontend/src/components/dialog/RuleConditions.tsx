import { useEffect, useState } from 'react'
import { Plus, X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { loadTagCounts } from '../../states/mailTags'
import { readsBody, RULE_FIELDS, valuedRuleField, type RuleCondition, type RuleField } from '../../states/mailRules'
import { RecipientInput } from '../composer/RecipientInput'
import { SelectInput } from '../field/Field'
import { inputClass } from '../search/SearchOptions'
import { SegmentedRow } from './AccountSettingsRows'

const FIELD_LABELS: Record<RuleField, string> = {
  from: 'composer.fields.from',
  to: 'composer.fields.to',
  subject: 'composer.fields.subject',
  body: 'rules.field.body',
  subject_or_body: 'rules.field.subjectOrBody',
  words: 'rules.field.words',
  tag: 'rules.field.tag',
  attachment: 'filters.hasAttachment',
  unread: 'filters.unread',
  starred: 'filters.starred',
}

/** A condition as the form holds it: `key` keeps a row's inputs its own while others come and go. */
export type DraftCondition = RuleCondition & { key: number }

let nextKey = 0

export const draftCondition = (
  condition: RuleCondition = { field: 'from', value: '', negate: false },
): DraftCondition => ({
  ...condition,
  key: nextKey++,
})

/** Whether a condition says anything: one left blank is dropped on save. */
export const conditionSet = ({ field, value }: RuleCondition) => !valuedRuleField(field) || !!value.trim()

/** A condition as the rule list sums it up. */
export function useConditionSummary() {
  const { t } = useTranslation()
  return (conditions: RuleCondition[], matchAny: boolean) => {
    const parts = conditions.filter(conditionSet).map((condition) => {
      const label = t(FIELD_LABELS[condition.field])
      const text = valuedRuleField(condition.field) ? `${label}: ${condition.value.trim()}` : label
      return condition.negate ? t('rules.summary.not', { condition: text }) : text
    })
    if (parts.length === 0) return t('rules.allMail')
    const joined = parts.join(' · ')
    return matchAny && parts.length > 1 ? t('rules.summaryAny', { conditions: joined }) : joined
  }
}

// One row per condition: what to look at, whether it should or shouldn't
// match, and the value, which depends on the field.
export function RuleConditions({
  conditions,
  matchAny,
  sent,
  onChange,
}: {
  conditions: DraftCondition[]
  matchAny: boolean
  /** For mail the user sends, which is acted on before any body is downloaded. */
  sent: boolean
  onChange: (conditions: DraftCondition[], matchAny: boolean) => void
}) {
  const { t } = useTranslation()
  // A tag is picked from those there are, as one mistyped would match nothing,
  // or everything when negated.
  const [tags, setTags] = useState<string[]>([])
  const wantsTags = conditions.some((condition) => condition.field === 'tag')
  useEffect(() => {
    if (!wantsTags) return
    void loadTagCounts().then(
      (counts) => setTags(counts.map(({ tag }) => tag).sort((a, b) => a.localeCompare(b))),
      () => setTags([]),
    )
  }, [wantsTags])
  const update = (at: number, patch: Partial<RuleCondition>) =>
    onChange(
      conditions.map((condition, index) => (index === at ? { ...condition, ...patch } : condition)),
      matchAny,
    )

  // A value only carries over between fields that take the same kind: text
  // can't become a tag, which has to be one there is.
  const kind = (field: RuleField) =>
    !valuedRuleField(field) ? '' : field === 'from' || field === 'to' ? 'address' : field === 'tag' ? 'tag' : 'text'
  const pickField = (at: number, field: RuleField) => {
    const was = conditions[at]
    const kept = kind(field) !== '' && kind(field) === kind(was.field)
    update(at, { field, value: kept ? was.value : '' })
  }

  const operators = (field: RuleField): [string, string] =>
    !valuedRuleField(field)
      ? [t('rules.op.yes'), t('rules.op.no')]
      : field === 'tag'
        ? [t('rules.op.is'), t('rules.op.isNot')]
        : [t('rules.op.contains'), t('rules.op.notContains')]

  return (
    <>
      {conditions.length > 1 && (
        <SegmentedRow
          title={t('rules.match')}
          value={matchAny ? 'any' : 'all'}
          options={[
            { value: 'all', label: t('rules.match.all') },
            { value: 'any', label: t('rules.match.any') },
          ]}
          onChange={(value) => onChange(conditions, value === 'any')}
        />
      )}
      <div className="flex flex-col gap-2 px-3.5 py-2.5">
        {conditions.map((condition, at) => {
          const label = t(FIELD_LABELS[condition.field])
          const [yes, no] = operators(condition.field)
          return (
            <div key={condition.key} data-condition={at} className="flex flex-wrap items-center gap-2">
              <SelectInput
                aria-label={t('rules.field')}
                value={condition.field}
                onChange={(event) => pickField(at, event.target.value as RuleField)}
                wrapperClassName="w-36 min-w-28 shrink"
                className="rounded-lg py-1.5 pl-2.5 text-xs"
              >
                {/* A row keeps the field it has, even one no new row is offered. */}
                {[
                  ...RULE_FIELDS.filter((field) => !(sent && readsBody(field))),
                  ...(RULE_FIELDS.includes(condition.field) ? [] : [condition.field]),
                ].map((field) => (
                  <option key={field} value={field} className="bg-chats text-primary">
                    {t(FIELD_LABELS[field])}
                  </option>
                ))}
              </SelectInput>
              <SelectInput
                aria-label={t('rules.op')}
                value={condition.negate ? 'not' : 'is'}
                onChange={(event) => update(at, { negate: event.target.value === 'not' })}
                wrapperClassName="w-36 min-w-28 shrink"
                className="rounded-lg py-1.5 pl-2.5 text-xs"
              >
                <option value="is" className="bg-chats text-primary">
                  {yes}
                </option>
                <option value="not" className="bg-chats text-primary">
                  {no}
                </option>
              </SelectInput>
              {condition.field === 'from' || condition.field === 'to' ? (
                <label className="flex min-w-40 flex-1">
                  <span className="sr-only">{label}</span>
                  <RecipientInput
                    bare
                    value={condition.value}
                    onChange={(value) => update(at, { value })}
                    accountId=""
                    className={`${inputClass} w-full`}
                  />
                </label>
              ) : condition.field === 'tag' ? (
                <SelectInput
                  aria-label={label}
                  value={condition.value}
                  onChange={(event) => update(at, { value: event.target.value })}
                  wrapperClassName="min-w-40 flex-1"
                  className="rounded-lg py-1.5 pl-2.5 text-xs"
                >
                  <option value="" disabled className="bg-chats text-secondary">
                    {t('rules.chooseTag')}
                  </option>
                  {/* One the list doesn't have yet stays as it was. */}
                  {(condition.value && !tags.includes(condition.value) ? [condition.value, ...tags] : tags).map(
                    (tag) => (
                      <option key={tag} value={tag} className="bg-chats text-primary">
                        {tag}
                      </option>
                    ),
                  )}
                </SelectInput>
              ) : valuedRuleField(condition.field) ? (
                // Narrow, it takes a line of its own rather than shrinking away.
                <div className="flex min-w-40 flex-1">
                  <input
                    aria-label={label}
                    value={condition.value}
                    onChange={(event) => update(at, { value: event.target.value })}
                    className={`${inputClass} w-full`}
                  />
                </div>
              ) : (
                <span className="flex-1" />
              )}
              <button
                type="button"
                aria-label={t('rules.removeCondition')}
                title={t('rules.removeCondition')}
                onClick={() =>
                  onChange(
                    conditions.filter((_, index) => index !== at),
                    matchAny,
                  )
                }
                className="flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded-md text-secondary transition-colors hover:bg-active hover:text-primary"
              >
                <X size={13} />
              </button>
            </div>
          )
        })}
        <button
          type="button"
          onClick={() => onChange([...conditions, draftCondition()], matchAny)}
          className="flex w-fit cursor-pointer items-center gap-1.5 rounded-md py-1 text-xs font-semibold text-accent hover:underline"
        >
          <Plus size={13} />
          {t('rules.addCondition')}
        </button>
        <p className="text-[0.6875rem] text-secondary">{t('rules.queryHint')}</p>
      </div>
    </>
  )
}
