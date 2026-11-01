import { invoke } from '../lib/bridge'

export type RuleMove = '' | 'archive' | 'trash' | 'folder'

export type Rule = {
  id: string
  query: string
  tags: string[]
  mark_read: boolean
  star: boolean
  move_to: RuleMove
  folder: string
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
