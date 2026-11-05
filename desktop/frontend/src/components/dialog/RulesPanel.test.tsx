import { afterEach, expect, it, spyOn } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import type { Rule, RuleStatus } from '../../states/mailRules'
import { EMPTY_FIELDS } from '../search/searchFields'
import { settleConfirm, ui$ } from '../../states/ui'
import type { Account } from '../../types'
import { RulesPanel } from './RulesPanel'

const bills: Rule = {
  id: 'r1',
  query: 'from:billing',
  tags: ['finance'],
  mark_read: true,
  star: false,
  move_to: 'archive',
  folder: '',
  folder_account: '',
  accounts: [],
  copy_folder: '',
  copy_account: '',
  disabled: false,
}

const ran: string[] = []
const stopped: string[] = []
let statusReply: RuleStatus = { log: [], running: {} }
let matches = 0

function stubCore(initial: Rule[], refuse = false) {
  const saved: Rule[][] = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          if (command === 'mail.rules') return { rules: initial }
          if (command === 'mail.ruleMatches') return { count: matches }
          if (command === 'mail.ruleStatus') return statusReply
          if (command === 'mail.stopRule') {
            stopped.push(payload.id)
            statusReply = { log: [], running: {} }
            return { ok: true }
          }
          if (command === 'mail.runRule') {
            ran.push(payload.id)
            return { count: matches }
          }
          if (command === 'search.parse') return { ...EMPTY_FIELDS, from: payload.query.replace('from:', '') }
          if (command === 'search.build') return { query: payload.fields.from && `from:${payload.fields.from}` }
          if (command !== 'mail.setRules') return { tags: [] }
          if (refuse) throw new Error('a rule needs an action')
          saved.push(payload.rules)
          return { rules: payload.rules.map((rule: Rule, at: number) => ({ ...rule, id: rule.id || `new${at}` })) }
        },
      },
    },
  }
  return saved
}

afterEach(() => {
  cleanup()
  delete (window as any).go
  accounts$.set([])
  ran.length = 0
  stopped.length = 0
  statusReply = { log: [], running: {} }
  matches = 0
  settleConfirm(false)
})

it('sums up what each rule does', async () => {
  stubCore([bills])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('From: billing')).toBeTruthy())
  expect(view.getByText('Tag finance · Mark as read · Move to Archive')).toBeTruthy()
  expect(view.queryByText('from:billing')).toBeNull()
})

it('keeps the form closed until asked for, and closes it on cancel', async () => {
  stubCore([])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('No rules yet.')).toBeTruthy())
  expect(view.queryByLabelText('From')).toBeNull()

  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))
  expect(view.getByLabelText('From')).toBeTruthy()
  expect(view.queryByText('No rules yet.')).toBeNull()

  fireEvent.click(view.getByRole('button', { name: 'Cancel' }))
  expect(view.queryByLabelText('From')).toBeNull()
})

it('adds a rule that only moves mail, warning before Trash', async () => {
  const saved = stubCore([])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('No rules yet.')).toBeTruthy())

  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))
  fireEvent.change(view.getByLabelText('From'), { target: { value: 'spam' } })
  fireEvent.click(view.getByRole('button', { name: 'Move to' }))
  fireEvent.click(view.getByText('Trash'))
  expect(view.getByText(/goes to Trash without asking/)).toBeTruthy()
  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))

  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0][0]).toMatchObject({ query: 'from:spam', tags: [], move_to: 'trash' })
  await waitFor(() => expect(view.getByText('From: spam')).toBeTruthy())
  expect(view.queryByLabelText('From')).toBeNull()
})

it('edits a rule in place', async () => {
  const saved = stubCore([bills, { ...bills, id: 'r2', query: 'from:other' }])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getAllByLabelText('Edit rule').length).toBe(2))

  fireEvent.click(view.getAllByLabelText('Edit rule')[0])
  await waitFor(() => expect((view.getByLabelText('From') as HTMLInputElement).value).toBe('billing'))
  expect(view.getByText('finance')).toBeTruthy()
  fireEvent.change(view.getByLabelText('From'), { target: { value: 'bills' } })
  fireEvent.click(view.getByRole('button', { name: 'Save' }))

  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0].map((rule) => [rule.id, rule.query, rule.tags])).toEqual([
    ['r1', 'from:bills', ['finance']],
    ['r2', 'from:other', ['finance']],
  ])
})

it('takes all new mail when no condition is filled in, warning before Trash', async () => {
  const saved = stubCore([])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('No rules yet.')).toBeTruthy())

  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))
  fireEvent.click(view.getByRole('button', { name: 'Move to' }))
  fireEvent.click(view.getByText('Trash'))
  expect(view.getByText(/All matching mail goes to Trash/)).toBeTruthy()
  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))

  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0][0].query).toBe('')
  await waitFor(() => expect(view.getByText('All mail')).toBeTruthy())
})

it('keeps the form and says why when the core refuses', async () => {
  stubCore([], true)
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('No rules yet.')).toBeTruthy())
  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))
  fireEvent.change(view.getByLabelText('From'), { target: { value: 'a' } })
  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))
  await waitFor(() => expect(view.getByText(/A rule needs at least one action/)).toBeTruthy())
  expect((view.getByLabelText('From') as HTMLInputElement).value).toBe('a')
})

it('runs on selected accounts only once one is picked', async () => {
  accounts$.set([
    { id: 'a', email: 'a@x.test', provider: 'imap' },
    { id: 'b', email: 'b@x.test', provider: 'imap' },
  ] as Account[])
  const saved = stubCore([])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('No rules yet.')).toBeTruthy())

  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))
  fireEvent.change(view.getByLabelText('From'), { target: { value: 'spam' } })
  fireEvent.click(view.getByRole('button', { name: 'Selected accounts' }))
  const add = view.getAllByRole('button', { name: 'Add rule' }).at(-1) as HTMLButtonElement
  expect(add.disabled).toBe(true)

  fireEvent.click(view.getByLabelText(/b@x\.test/))
  fireEvent.click(view.getByRole('button', { name: 'Move to' }))
  fireEvent.click(view.getByText('Archive'))
  fireEvent.click(add)

  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0][0].accounts).toEqual(['b'])
})

it('shows the accounts of a scoped rule by their avatars', async () => {
  accounts$.set([
    { id: 'a', email: 'a@x.test', provider: 'imap' },
    { id: 'b', email: 'b@x.test', provider: 'imap' },
  ] as Account[])
  stubCore([{ ...bills, accounts: ['b'] }])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('From: billing')).toBeTruthy())
  expect(view.getByTitle('b@x.test')).toBeTruthy()
  expect(view.queryByTitle('a@x.test')).toBeNull()
})

it('runs on existing mail only after showing how much and asking', async () => {
  stubCore([bills])
  matches = 12
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('From: billing')).toBeTruthy())

  fireEvent.click(view.getByLabelText('Run on existing mail'))
  await waitFor(() => expect(ui$.confirm.get()?.message).toContain('12'))
  expect(ran).toEqual([])

  settleConfirm(true)
  await waitFor(() => expect(ran).toEqual(['r1']))
})

it('deletes a rule only once confirmed', async () => {
  const saved = stubCore([bills])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('From: billing')).toBeTruthy())

  fireEvent.click(view.getByLabelText('Delete rule'))
  await waitFor(() => expect(ui$.confirm.get()?.message).toContain('From: billing'))
  settleConfirm(false)
  await waitFor(() => expect(ui$.confirm.get()).toBeNull())
  expect(saved).toEqual([])

  fireEvent.click(view.getByLabelText('Delete rule'))
  await waitFor(() => expect(ui$.confirm.get()).toBeTruthy())
  settleConfirm(true)
  await waitFor(() => expect(saved).toEqual([[]]))
})

it('does not ask when nothing matches, and leaves the mail alone', async () => {
  stubCore([bills])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('From: billing')).toBeTruthy())

  fireEvent.click(view.getByLabelText('Run on existing mail'))
  await waitFor(() => expect(ui$.toast.get()).toBe('No matching mail in the Inbox.'))
  expect(ran).toEqual([])
})

const entry = { at: 1, rule_id: 'r1', account: 'a', subject: 'Invoice', count: 1, failed: false, detail: '' }

it('turns a rule off and back on without losing it', async () => {
  const saved = stubCore([bills])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('From: billing')).toBeTruthy())

  fireEvent.click(view.getByRole('switch', { name: 'Rule on' }))
  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0][0]).toMatchObject({ id: 'r1', query: 'from:billing', disabled: true })
  await waitFor(() => expect((view.getByLabelText('Run on existing mail') as HTMLButtonElement).disabled).toBe(true))
})

it('opens the rule asked for last, whichever answer comes back first', async () => {
  stubCore([bills, { ...bills, id: 'r2', query: 'from:other' }])
  const slow = (window as any).go.main.App.Invoke
  ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
    if (command === 'search.parse' && payload.query === 'from:billing' && edits++ > 0) {
      await new Promise((resolve) => setTimeout(resolve, 50))
    }
    return slow(command, payload)
  }
  let edits = 0
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getAllByLabelText('Edit rule').length).toBe(2))

  fireEvent.click(view.getAllByLabelText('Edit rule')[0])
  fireEvent.click(view.getAllByLabelText('Edit rule')[1])
  await new Promise((resolve) => setTimeout(resolve, 100))
  expect((view.getByLabelText('From') as HTMLInputElement).value).toBe('other')
})

it('reorders rules, since the first to move a message wins', async () => {
  const saved = stubCore([bills, { ...bills, id: 'r2', query: 'from:other' }])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getAllByLabelText('Drag to reorder').length).toBe(2))
  const handle = view.getAllByLabelText('Drag to reorder')[0]
  handle.focus()

  fireEvent.keyDown(handle, { key: 'ArrowUp' })
  expect(saved.length).toBe(0)

  fireEvent.keyDown(handle, { key: 'ArrowDown' })
  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0].map((rule) => rule.id)).toEqual(['r2', 'r1'])
  expect(document.activeElement).toBe(view.getAllByLabelText('Drag to reorder')[1])
})

async function dragFirstRuleDown(refuse: boolean) {
  const saved = stubCore([bills, { ...bills, id: 'r2', query: 'from:other' }], refuse)
  const rect = spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    const row = this.closest('[data-rule]')
    const top = row ? [...row.parentElement!.children].indexOf(row) * 40 : 0
    return new DOMRect(0, top, 200, 40)
  })
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getAllByLabelText('Drag to reorder').length).toBe(2))
  const handle = view.getAllByLabelText('Drag to reorder')[0]

  fireEvent.mouseDown(handle, { clientX: 10, clientY: 10, button: 0 })
  fireEvent.mouseMove(document, { clientX: 10, clientY: 30 })
  fireEvent.mouseMove(document, { clientX: 10, clientY: 55 })
  await waitFor(() => expect(handle.closest('[data-rule]')!.className).toContain('bg-raised'))
  fireEvent.mouseUp(document)
  rect.mockRestore()
  // dnd-kit retains its document click listener for 50ms after dragging.
  await new Promise((resolve) => setTimeout(resolve, 60))
  return saved
}

const shownOrder = () => [...document.querySelectorAll('[data-rule]')].map((row) => row.getAttribute('data-rule'))

it('saves the order a rule is dragged to', async () => {
  const saved = await dragFirstRuleDown(false)
  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0].map((rule) => rule.id)).toEqual(['r2', 'r1'])
  await waitFor(() => expect(shownOrder()).toEqual(['r2', 'r1']))
})

it('puts a dragged rule back when the save fails', async () => {
  await dragFirstRuleDown(true)
  await waitFor(() => expect(shownOrder()).toEqual(['r1', 'r2']))
})

it('shows what a run has left and stops it', async () => {
  stubCore([bills])
  statusReply = { log: [], running: { r1: 40 } }
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText(/Messages left: 40/)).toBeTruthy())
  expect(view.queryByLabelText('Run on existing mail')).toBeNull()

  fireEvent.click(view.getByLabelText('Stop'))
  await waitFor(() => expect(stopped).toEqual(['r1']))
  await waitFor(() => expect(view.getByLabelText('Run on existing mail')).toBeTruthy())
})

it('says when a rule last failed, and lists what rules did', async () => {
  stubCore([bills])
  statusReply = {
    log: [
      { ...entry, failed: true, detail: 'no such folder' },
      { ...entry, subject: '', count: 12 },
    ],
    running: {},
  }
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('Last run failed')).toBeTruthy())
  expect(view.getAllByTitle('no such folder').length).toBe(2)
  expect(view.getByText('Invoice')).toBeTruthy()
  expect(view.getByText(/Messages: 12/)).toBeTruthy()
})

it('lets a rule whose only account was removed move to the one that is left', async () => {
  accounts$.set([{ id: 'a', email: 'a@x.test', provider: 'imap' }] as Account[])
  const saved = stubCore([{ ...bills, disabled: true, accounts: ['gone'] }])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByLabelText('Edit rule')).toBeTruthy())

  fireEvent.click(view.getByLabelText('Edit rule'))
  await waitFor(() => expect(view.getByLabelText(/a@x\.test/)).toBeTruthy())
  fireEvent.click(view.getByLabelText(/a@x\.test/))
  fireEvent.click(view.getByRole('button', { name: 'Save' }))

  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0][0].accounts).toEqual(['a'])
})

it('says when a rule uses a removed account, and keeps it off', async () => {
  accounts$.set([{ id: 'a', email: 'a@x.test', provider: 'imap' }] as Account[])
  const saved = stubCore([{ ...bills, disabled: true, copy_account: 'gone', copy_folder: 'Saved' }])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText(/account this rule uses was removed/)).toBeTruthy())

  fireEvent.click(view.getByRole('switch', { name: 'Rule on' }))
  expect(saved).toEqual([])
})

it('saves and edits a five-minute wait after reading', async () => {
  const saved = stubCore([bills])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('From: billing')).toBeTruthy())
  fireEvent.click(view.getByRole('button', { name: 'Edit rule' }))
  await waitFor(() => expect(view.getByLabelText('When to run')).toBeTruthy())
  expect(view.queryByLabelText('Wait')).toBeNull()
  fireEvent.change(view.getByLabelText('When to run'), { target: { value: 'read' } })
  expect((view.getByLabelText('Wait') as HTMLInputElement).value).toBe('5')
  fireEvent.click(view.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0][0].trigger).toBe('read')
  expect(saved[0][0].delay_seconds).toBe(300)
  await waitFor(() => expect(view.getByText(/5 minutes after it's read/)).toBeTruthy())
  fireEvent.click(view.getByRole('button', { name: 'Edit rule' }))
  await waitFor(() => expect((view.getByLabelText('Wait') as HTMLInputElement).value).toBe('5'))
  expect((view.getByLabelText('When to run') as HTMLSelectElement).value).toBe('read')
})

it('shows an age rule in days and blocks saving an empty age', async () => {
  const saved = stubCore([{ ...bills, trigger: 'age', delay_seconds: 30 * 86400 }])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText(/Older than 30 days/)).toBeTruthy())
  fireEvent.click(view.getByRole('button', { name: 'Edit rule' }))
  await waitFor(() => expect((view.getByLabelText('Age') as HTMLInputElement).value).toBe('30'))
  expect((view.getByLabelText('Unit') as HTMLSelectElement).value).toBe('days')
  fireEvent.change(view.getByLabelText('Age'), { target: { value: '' } })
  expect((view.getByRole('button', { name: 'Save' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.change(view.getByLabelText('Age'), { target: { value: '2' } })
  fireEvent.change(view.getByLabelText('Unit'), { target: { value: 'hours' } })
  fireEvent.click(view.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0][0].delay_seconds).toBe(7200)
})
