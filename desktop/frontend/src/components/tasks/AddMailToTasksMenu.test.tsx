import { afterEach, beforeEach, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { AddMailToTasksMenu } from './AddMailToTasksMenu'
import { ui$ } from '../../states/ui'

let lists: { id: string; title: string }[]
let creates: Record<string, unknown>[]
let closed: boolean
beforeEach(() => {
  lists = [
    { id: 'first', title: 'First list' },
    { id: 'second', title: 'Second list' },
  ]
  creates = []
  closed = false
  ui$.activeTaskList.set('first')
  ui$.tasksPanelOpen.set(false)
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: Record<string, unknown>) => {
          if (command === 'tasks.lists') return { lists, default_list_id: 'first' }
          if (command === 'tasks.create') creates.push(payload)
          return {}
        },
      },
    },
  }
})
afterEach(cleanup)

function menu() {
  return render(
    <AddMailToTasksMenu
      title="Follow up"
      account="account"
      threadId="thread"
      onAdded={() => {
        closed = true
      }}
    />,
  )
}

it('waits for a list choice and sends the explicit destination', async () => {
  const view = menu()
  const add = view.getByRole('button', { name: 'Add to Tasks' }) as HTMLButtonElement
  await waitFor(() => expect(add.disabled).toBe(false))
  fireEvent.click(add)
  expect(creates).toHaveLength(0)
  fireEvent.click(view.getByRole('button', { name: 'Second list' }))
  await waitFor(() => expect(creates).toHaveLength(1))
  expect(creates[0]).toMatchObject({ list_id: 'second', account: 'account', thread_id: 'thread' })
  expect(closed).toBe(true)
})

it('adds directly when there is only one list', async () => {
  lists = [{ id: 'only', title: 'Only list' }]
  const view = menu()
  const add = view.getByRole('button', { name: 'Add to Tasks' }) as HTMLButtonElement
  await waitFor(() => expect(add.disabled).toBe(false))
  fireEvent.click(add)
  await waitFor(() => expect(creates).toHaveLength(1))
  expect(creates[0].list_id).toBe('only')
  expect(view.queryByRole('button', { name: 'Only list' })).toBeNull()
})

it('does not create a task when dismissed without choosing', async () => {
  const view = menu()
  const add = view.getByRole('button', { name: 'Add to Tasks' }) as HTMLButtonElement
  await waitFor(() => expect(add.disabled).toBe(false))
  fireEvent.click(add)
  view.unmount()
  expect(creates).toHaveLength(0)
})
