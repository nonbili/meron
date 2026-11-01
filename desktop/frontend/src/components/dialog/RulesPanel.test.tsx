import { afterEach, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import type { Rule } from '../../states/mailRules'
import { RulesPanel } from './RulesPanel'

const bills: Rule = {
  id: 'r1',
  query: 'from:billing',
  tags: ['finance'],
  mark_read: true,
  star: false,
  move_to: 'archive',
  folder: '',
}

function stubCore(initial: Rule[], refuse = false) {
  const saved: Rule[][] = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: { rules: Rule[] }) => {
          if (command === 'mail.rules') return { rules: initial }
          if (refuse) throw new Error('a rule needs an action')
          saved.push(payload.rules)
          return { rules: payload.rules.map((rule, at) => ({ ...rule, id: rule.id || `new${at}` })) }
        },
      },
    },
  }
  return saved
}

afterEach(() => {
  cleanup()
  delete (window as any).go
})

it('sums up what each rule does', async () => {
  stubCore([bills])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('finance · Mark as read · Archive')).toBeTruthy())
})

it('adds a rule that only moves mail, warning before Trash', async () => {
  const saved = stubCore([])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('No rules yet.')).toBeTruthy())

  fireEvent.change(view.getByLabelText('Search'), { target: { value: 'from:spam' } })
  fireEvent.change(view.getByRole('combobox'), { target: { value: 'trash' } })
  expect(view.getByText(/goes to Trash without asking/)).toBeTruthy()
  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))

  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0][0]).toMatchObject({ query: 'from:spam', tags: [], move_to: 'trash' })
  await waitFor(() => expect(view.getByText('from:spam')).toBeTruthy())
})

it('edits a rule in place', async () => {
  const saved = stubCore([bills, { ...bills, id: 'r2', query: 'from:other' }])
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getAllByLabelText('Edit rule').length).toBe(2))

  fireEvent.click(view.getAllByLabelText('Edit rule')[0])
  expect((view.getByLabelText('Tags, separated by commas') as HTMLInputElement).value).toBe('finance')
  fireEvent.change(view.getByLabelText('Search'), { target: { value: 'from:bills' } })
  fireEvent.click(view.getByRole('button', { name: 'Save' }))

  await waitFor(() => expect(saved.length).toBe(1))
  expect(saved[0].map((rule) => [rule.id, rule.query])).toEqual([
    ['r1', 'from:bills'],
    ['r2', 'from:other'],
  ])
})

it('keeps the form and says why when the core refuses', async () => {
  stubCore([], true)
  const view = render(<RulesPanel />)
  await waitFor(() => expect(view.getByText('No rules yet.')).toBeTruthy())
  fireEvent.change(view.getByLabelText('Search'), { target: { value: 'from:a' } })
  fireEvent.click(view.getByRole('button', { name: 'Add rule' }))
  await waitFor(() => expect(view.getByText(/A rule needs a search/)).toBeTruthy())
  expect((view.getByLabelText('Search') as HTMLInputElement).value).toBe('from:a')
})
