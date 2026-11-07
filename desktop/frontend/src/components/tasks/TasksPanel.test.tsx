import { afterEach, expect, it } from 'bun:test'
import { act, cleanup, render, waitFor } from '@testing-library/react'
import { ui$ } from '../../states/ui'
import { TasksPanel } from './TasksPanel'
import { tasks$, type Task } from '../../states/tasks'

afterEach(cleanup)

it('refreshes an open list when mail is added from another window', async () => {
  const handlers = new Map<string, (detail?: unknown) => void>()
  let stored: Task[] = []
  ui$.activeTaskList.set('list')
  tasks$.lists.set([{ id: 'list', title: 'Tasks' }])
  tasks$.items.set([])
  ;(window as any).runtime = {
    EventsOn: (name: string, callback: (detail?: unknown) => void) => {
      handlers.set(name, callback)
      return () => handlers.delete(name)
    },
  }
  ;(window as any).go = {
    main: { App: { Invoke: async (command: string) => (command === 'tasks.items' ? { tasks: stored } : {}) } },
  }
  const view = render(<TasksPanel listId="list" />)
  await waitFor(() => expect(handlers.has('tasks.changed')).toBe(true))
  stored = [
    {
      id: 'task',
      list_id: 'list',
      title: 'Reply from the other window',
      notes: '',
      due_at: 0,
      completed_at: 0,
      done: false,
      account: 'a',
      thread_id: 'thread',
      message_id: '',
    },
  ]
  act(() => handlers.get('tasks.changed')?.({ method: 'tasks.create' }))
  await waitFor(() => expect(view.getByText('Reply from the other window')).toBeTruthy())
})
