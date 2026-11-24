import { invoke } from '../lib/bridge'

export type RuleTrigger = 'arrival' | 'read' | 'age'

export type RuleMove = '' | 'archive' | 'trash' | 'folder'

/**
 * From, To and Subject take comma-separated alternatives; the last three take no
 * value. Body and Subject or body wait for the message to be downloaded. Words is
 * search text, where a rule from before rows keeps what no row could say.
 */
export type RuleField =
  'from' | 'to' | 'subject' | 'body' | 'subject_or_body' | 'words' | 'tag' | 'attachment' | 'unread' | 'starred'

/** The fields a new row may take: Words only ever comes from an older rule. */
export const RULE_FIELDS: RuleField[] = [
  'from',
  'to',
  'subject',
  'body',
  'subject_or_body',
  'tag',
  'attachment',
  'unread',
  'starred',
]

/** Whether a field reads the body, which a rule for sent mail can't. */
export const readsBody = (field: RuleField) => field === 'body' || field === 'subject_or_body'

export const valuedRuleField = (field: RuleField) => !['attachment', 'unread', 'starred'].includes(field)

export type RuleCondition = {
  field: RuleField
  value: string
  /** Matches the mail the condition otherwise wouldn't. */
  negate: boolean
}

export type Rule = {
  id: string
  /** None for all mail. */
  conditions: RuleCondition[]
  /** Any one condition is enough, rather than all of them. */
  match_any: boolean
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
  /** Addresses to forward matching mail to, comma-separated; empty for none. */
  forward_to?: string
  /** Kept, but not run. */
  disabled: boolean
  /** Runs on mail the user sent, as it shows up in Sent, instead of on mail that arrives. */
  sent?: boolean
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

/** How many messages already in the Inbox, or in Sent for a rule for sent mail, a saved rule would run on. */
export async function ruleMatches(id: string): Promise<{ count: number; withoutBody: number }> {
  const { count, without_body } = await invoke<{ count: number; without_body?: number }>('mail.ruleMatches', { id })
  return { count, withoutBody: without_body ?? 0 }
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
  return (rules ?? []).map((rule) => ({ ...rule, conditions: rule.conditions ?? [], match_any: !!rule.match_any }))
}

// The core normalizes what it keeps and refuses a rule that could never apply.
export async function saveRules(rules: Rule[]): Promise<Rule[]> {
  const saved = await invoke<{ rules: Rule[] }>('mail.setRules', { rules })
  return saved.rules ?? []
}
