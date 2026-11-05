import { EMPTY_FIELDS, type SearchFields } from '../components/search/searchFields'
import { invoke } from '../lib/bridge'

export type RuleTrigger = 'arrival' | 'read' | 'age'

export type RuleMove = '' | 'archive' | 'trash' | 'folder'

export type Rule = {
  id: string
  query: string
  trigger?: RuleTrigger
  delay_seconds?: number
  tags: string[]
  mark_read: boolean
  star: boolean
  move_to: RuleMove
  folder: string
  /** The account of `folder`; empty for the mail's own account. */
  folder_account: string
  /** The accounts the rule runs on; empty for all of them. */
  accounts: string[]
  copy_folder: string
  copy_account: string
  /** Kept, but not run. */
  disabled: boolean
}

export type RuleLogEntry = {
  /** Unix seconds. */
  at: number
  rule_id: string
  account: string
  /** The message's subject; empty for a run by hand, which logs one line. */
  subject: string
  count: number
  failed: boolean
  detail: string
}

export type RuleStatus = {
  log: RuleLogEntry[]
  /** How many messages each rule run by hand has left. */
  running: Record<string, number>
}

// A rule keeps its condition as search text; people only ever see it as fields.
// Text the core can't read stays in the words, so a save doesn't lose it.
export async function ruleFields(query: string): Promise<SearchFields> {
  const parsed = await invoke<SearchFields>('search.parse', { query }).catch(() => null)
  return { ...EMPTY_FIELDS, ...parsed, words: parsed ? parsed.words : query }
}

export async function ruleQuery(fields: SearchFields): Promise<string> {
  const { query } = await invoke<{ query: string }>('search.build', { fields })
  return query
}

/** How many messages already in the Inbox a saved rule would run on. */
export async function ruleMatches(id: string): Promise<number> {
  const { count } = await invoke<{ count: number }>('mail.ruleMatches', { id })
  return count
}

/** Runs a saved rule on the mail already in the Inbox; the work goes on in the background. */
export async function runRule(id: string): Promise<number> {
  const { count } = await invoke<{ count: number }>('mail.runRule', { id })
  return count
}

export async function loadRuleStatus(): Promise<RuleStatus> {
  const status = await invoke<Partial<RuleStatus>>('mail.ruleStatus', {})
  return { log: status.log ?? [], running: status.running ?? {} }
}

/** Drops what is left of a rule run by hand; what is done stays done. */
export async function stopRule(id: string): Promise<void> {
  await invoke('mail.stopRule', { id })
}

export async function loadRules(): Promise<Rule[]> {
  const { rules } = await invoke<{ rules: Rule[] }>('mail.rules', {})
  return rules ?? []
}

// The core normalizes what it keeps and refuses a rule that could never apply.
export async function saveRules(rules: Rule[]): Promise<Rule[]> {
  const saved = await invoke<{ rules: Rule[] }>('mail.setRules', { rules })
  return saved.rules ?? []
}
