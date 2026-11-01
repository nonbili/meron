import { afterEach, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { useEscapeKey } from '../../lib/useEscapeKey'
import type { Message } from '../../types'
import { kanban$, kanbanBoardColumnKey, kanbanColumnKey, renameKanbanColumnQueries } from '../../states/kanban'
import { mail$ } from '../../states/mail'
import { replaceTag } from '../../states/mailTags'
import { settings$ } from '../../states/settings'
import { settleConfirm, ui$ } from '../../states/ui'
import { TagList, TagsSection } from './TagsSection'

type Call = { command: string; payload: { tag?: string; to?: string; queries?: string[] } }

function stubCore() {
  const calls: Call[] = []
  let counts = [
    { tag: 'rare', count: 1 },
    { tag: 'work', count: 5 },
  ]
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: Call['payload']) => {
          calls.push({ command, payload })
          if (command === 'mail.tagList') return { tags: counts }
          if (command === 'mail.rules') return { rules: [] }
          if (command !== 'mail.replaceTag') return {}
          counts = counts.filter(({ tag }) => tag !== payload.tag)
          return {
            ok: true,
            tag: payload.to?.trim().toLowerCase() ?? null,
            rules: [],
            queries: payload.queries?.map(() => 'tag:core-spelling'),
          }
        },
      },
    },
  }
  return calls
}

afterEach(() => {
  cleanup()
  delete (window as any).go
  mail$.threads.set([])
  mail$.messages.set([])
  kanban$.threads.set({})
  kanban$.searchScope.set('all')
  kanban$.activeBoardId.set('')
  kanban$.filters.set({})
  kanban$.paneColumnKey.set('')
  settings$.kanbanMinimizedColumns.set({})
  settings$.kanbanBoards.set([])
  ui$.confirm.set(null)
})

it('keeps columns on a rename, folding two that come to mean one search', () => {
  const column = (query: string) => ({ accountId: 'a', folderId: 'INBOX', query })
  settings$.kanbanBoards.set([{ id: 'b', name: 'B', columns: [column('tag:old'), column('tag:new'), column('tag:x')] }])
  kanban$.searchScope.set(kanbanColumnKey(column('tag:old')))
  renameKanbanColumnQueries(new Map([['tag:old', 'tag:new']]))
  expect(settings$.kanbanBoards.get()[0].columns.map(({ query }) => query)).toEqual(['tag:new', 'tag:x'])
  expect(kanban$.searchScope.get()).toBe(kanbanColumnKey(column('tag:new')))
})

it("carries a renamed column's own state to its new name", () => {
  const column = { accountId: 'a', folderId: 'INBOX', query: 'tag:old' }
  const target = { ...column, query: 'tag:new' }
  settings$.kanbanBoards.set([{ id: 'b', name: 'B', columns: [column] }])
  settings$.kanbanMinimizedColumns[kanbanBoardColumnKey('b', column)].set(true)
  kanban$.filters[kanbanColumnKey(column)].set('unread')
  kanban$.paneColumnKey.set(kanbanBoardColumnKey('b', column))

  renameKanbanColumnQueries(new Map([['tag:old', 'tag:new']]))

  expect(settings$.kanbanMinimizedColumns[kanbanBoardColumnKey('b', target)].get()).toBe(true)
  expect(settings$.kanbanMinimizedColumns[kanbanBoardColumnKey('b', column)].get()).toBeUndefined()
  expect(kanban$.filters[kanbanColumnKey(target)].get()).toBe('unread')
  expect(kanban$.paneColumnKey.get()).toBe(kanbanBoardColumnKey('b', target))
})

it('sends saved searches through the core and takes back what it answers', async () => {
  const calls = stubCore()
  const column = { accountId: 'a', folderId: 'INBOX', query: 'tag:old' }
  settings$.kanbanBoards.set([{ id: 'b', name: 'B', columns: [column] }])
  await replaceTag('old', 'new')
  expect(calls.find(({ command }) => command === 'mail.replaceTag')?.payload.queries).toEqual(['tag:old'])
  expect(settings$.kanbanBoards.get()[0].columns[0].query).toBe('tag:core-spelling')
})

it('updates what is on screen from the name the core kept', async () => {
  stubCore()
  // On a board the list isn't read again, leaving the update itself to look at.
  kanban$.activeBoardId.set('board')
  mail$.threads.set([{ thread_id: 't', tags: ['keep', 'old'] }, { thread_id: 'u' }] as Message[])
  kanban$.threads.set({ column: [{ thread_id: 't', tags: ['new', 'old'] }] as Message[] })

  await replaceTag('old', ' New ')
  expect(mail$.threads.get()[0].tags).toEqual(['keep', 'new'])
  expect(kanban$.threads.get().column[0].tags).toEqual(['new'])

  await replaceTag('new')
  expect(mail$.threads.get()[0].tags).toEqual(['keep'])
  expect(kanban$.threads.get().column[0].tags).toEqual([])
})

it('reads the list again after a replacement', async () => {
  const calls = stubCore()
  await replaceTag('old', 'new')
  const reads = () => calls.filter(({ command }) => !['mail.replaceTag', 'mail.tagList'].includes(command))
  await waitFor(() => expect(reads().length).toBeGreaterThan(0))
})

it('keeps the long list behind a row, so the page stays short', async () => {
  stubCore()
  const view = render(<TagsSection />)
  expect(view.queryByText('work')).toBeNull()
  fireEvent.click(view.getByRole('button', { name: 'Tags' }))
  await waitFor(() => expect(view.getByText('work')).toBeTruthy())
  fireEvent.click(view.getByLabelText('Close'))
  await waitFor(() => expect(view.queryByText('work')).toBeNull())
})

it('renames a tag from its row, most used first', async () => {
  const calls = stubCore()
  const view = render(<TagList />)
  await waitFor(() => expect(view.getByText('work')).toBeTruthy())
  expect(view.getAllByLabelText('Rename tag').length).toBe(2)
  expect(view.getByText('work').compareDocumentPosition(view.getByText('rare'))).toBe(Node.DOCUMENT_POSITION_FOLLOWING)

  fireEvent.click(view.getAllByLabelText('Rename tag')[0])
  const input = view.getByRole('textbox', { name: 'Rename tag' }) as HTMLInputElement
  expect(input.value).toBe('work')
  fireEvent.change(input, { target: { value: 'jobs' } })
  fireEvent.keyDown(input, { key: 'Enter' })

  await waitFor(() => expect(view.queryByText('work')).toBeNull())
  expect(calls.find(({ command }) => command === 'mail.replaceTag')?.payload).toEqual({
    tag: 'work',
    to: 'jobs',
    queries: [],
  })
})

it('leaves a rename on Escape without closing what is underneath', async () => {
  stubCore()
  let closed = 0
  function SettingsBelow() {
    useEscapeKey(() => closed++)
    return null
  }
  const view = render(
    <>
      <SettingsBelow />
      <TagList />
    </>,
  )
  await waitFor(() => expect(view.getByText('work')).toBeTruthy())
  fireEvent.click(view.getAllByLabelText('Rename tag')[0])
  fireEvent.keyDown(view.getByRole('textbox', { name: 'Rename tag' }), { key: 'Escape' })
  await waitFor(() => expect(view.queryByRole('textbox', { name: 'Rename tag' })).toBeNull())
  expect(view.queryByRole('textbox', { name: 'Rename tag' })).toBeNull()
  expect(closed).toBe(0)
})

it('filters the list by what is typed, ignoring case', async () => {
  stubCore()
  const view = render(<TagList />)
  await waitFor(() => expect(view.getByText('work')).toBeTruthy())
  fireEvent.change(view.getByLabelText('Filter tags'), { target: { value: ' WO ' } })
  expect(view.getByText('work')).toBeTruthy()
  expect(view.queryByText('rare')).toBeNull()
})

it('asks before deleting, and does nothing when told not to', async () => {
  const calls = stubCore()
  const view = render(<TagList />)
  await waitFor(() => expect(view.getByText('work')).toBeTruthy())
  const replaced = () => calls.filter(({ command }) => command === 'mail.replaceTag')

  fireEvent.click(view.getAllByLabelText('Delete tag')[0])
  await waitFor(() => expect(ui$.confirm.get()?.message).toContain('5'))
  settleConfirm(false)
  await new Promise((resolve) => setTimeout(resolve))
  expect(replaced()).toEqual([])

  fireEvent.click(view.getAllByLabelText('Delete tag')[0])
  await waitFor(() => expect(ui$.confirm.get()).toBeTruthy())
  settleConfirm(true)
  await waitFor(() => expect(view.queryByText('work')).toBeNull())
  expect(replaced().map(({ payload }) => payload)).toEqual([{ tag: 'work', to: undefined, queries: [] }])
})
