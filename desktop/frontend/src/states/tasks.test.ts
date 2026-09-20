import { beforeEach, describe, expect, it } from 'bun:test'

import { ui$ } from './ui'
import { settings$, setTasksEnabled } from './settings'
import {
  addTaskFromMessage,
  clearCompletedTasks,
  deleteTask,
  deleteTaskList,
  closeTasksPanel,
  loadTasks,
  openTasksPanel,
  restoreTasksSession,
  setShowCompleted,
  tasks$,
  toggleTasksPanel,
  type Task,
} from './tasks'

const task = (overrides: Partial<Task> = {}): Task => ({
  id: 'task-1',
  list_id: 'list-1',
  title: 'Reply to the landlord',
  notes: '',
  due_at: 0,
  completed_at: 0,
  done: false,
  account: '',
  thread_id: '',
  message_id: '',
  ...overrides,
})

const calls: { command: string; payload: any }[] = []
let items: Task[] = []

beforeEach(() => {
  calls.length = 0
  items = [task()]
  ui$.tasksPanelOpen.set(false)
  ui$.activeTaskList.set('')
  ui$.toast.set('')
  ui$.toastUndo.set(null)
  tasks$.lists.set([])
  tasks$.items.set([])
  tasks$.showCompleted.set(false)
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push({ command, payload })
          if (command === 'tasks.lists') {
            return {
              lists: [
                { id: 'list-1', title: 'My Tasks' },
                { id: 'list-2', title: 'Groceries' },
              ],
              default_list_id: 'list-1',
            }
          }
          if (command === 'tasks.items') return { tasks: items }
          if (command === 'tasks.delete') return { ok: true, restore: { tasks: [task()] } }
          if (command === 'tasks.listDelete') {
            return { ok: true, restore: { list: { id: 'list-1' }, tasks: [task()] } }
          }
          if (command === 'tasks.clearCompleted') {
            return { ok: true, removed: 2, restore: { tasks: [task({ done: true })] } }
          }
          return { ok: true }
        },
      },
    },
  }
})

describe('openTasksPanel', () => {
  it('shows the panel on the core default list when none is remembered', async () => {
    await openTasksPanel()
    expect(ui$.tasksPanelOpen.get()).toBe(true)
    expect(ui$.activeTaskList.get()).toBe('list-1')
    expect(tasks$.items.get()).toHaveLength(1)
  })

  it('honours an explicitly requested list', async () => {
    await openTasksPanel('list-2')
    expect(ui$.activeTaskList.get()).toBe('list-2')
  })

  // A list can be deleted on another device, or by the user before a restart
  // restores the id: falling back beats opening a list that isn't there.
  it('falls back to the default when the requested list is gone', async () => {
    await openTasksPanel('list-deleted')
    expect(ui$.activeTaskList.get()).toBe('list-1')
  })

  it('reopens the list the previous session left off on', async () => {
    ui$.activeTaskList.set('list-2')
    await openTasksPanel()
    expect(ui$.activeTaskList.get()).toBe('list-2')
  })

  // The panel is chrome beside the main view, not a view of its own, so
  // opening it must leave the mailbox selection exactly where it was.
  it('does not disturb the open thread or mailbox', async () => {
    ui$.selectedAccount.set('acct1')
    ui$.selectedThread.set('acct1#INBOX#4')
    await openTasksPanel()
    expect(ui$.selectedAccount.get()).toBe('acct1')
    expect(ui$.selectedThread.get()).toBe('acct1#INBOX#4')
  })
})

describe('loadTasks', () => {
  it('asks for completed tasks only when they are shown', async () => {
    ui$.activeTaskList.set('list-1')
    await loadTasks()
    expect(calls.at(-1)?.payload.include_completed).toBe(false)

    await setShowCompleted(true)
    expect(calls.at(-1)?.payload.include_completed).toBe(true)
  })

  // Clicking through lists faster than the core answers must not paint the
  // first list's tasks over the second list's.
  it('drops a response for a list that is no longer open', async () => {
    ui$.activeTaskList.set('list-1')
    const pending = loadTasks('list-1')
    ui$.activeTaskList.set('list-2')
    await pending
    expect(tasks$.items.get()).toHaveLength(0)
  })

  it('does nothing without an open list', async () => {
    await loadTasks('')
    expect(calls.some((call) => call.command === 'tasks.items')).toBe(false)
  })
})

describe('toggleTasksPanel', () => {
  it('opens when hidden and hides when showing', async () => {
    toggleTasksPanel()
    // The open path reads the lists, so let it settle before asserting.
    await Promise.resolve()
    await Promise.resolve()
    expect(ui$.tasksPanelOpen.get()).toBe(true)

    toggleTasksPanel()
    expect(ui$.tasksPanelOpen.get()).toBe(false)
  })
})

describe('addTaskFromMessage', () => {
  it('uses the default on first use and keeps the mail link', async () => {
    await addTaskFromMessage({
      title: 'Boiler repair',
      account: 'acct1',
      threadId: 'acct1#thread#4',
      messageId: '<boiler@example.com>',
    })
    const create = calls.find((call) => call.command === 'tasks.create')
    expect(create?.payload).toEqual({
      list_id: '',
      title: 'Boiler repair',
      account: 'acct1',
      thread_id: 'acct1#thread#4',
      message_id: '<boiler@example.com>',
    })
  })

  for (const panelOpen of [true, false]) {
    it(`uses the selected list with the panel ${panelOpen ? 'open' : 'closed'}`, async () => {
      await openTasksPanel('list-2')
      if (!panelOpen) closeTasksPanel()
      calls.length = 0
      await addTaskFromMessage({ title: 'Follow up', account: 'acct1', threadId: 'thread-1' })
      expect(calls.find((call) => call.command === 'tasks.create')?.payload.list_id).toBe('list-2')
      expect(calls.some((call) => call.command === 'tasks.items')).toBe(panelOpen)
      if (panelOpen) {
        expect(calls.find((call) => call.command === 'tasks.items')?.payload.list_id).toBe('list-2')
      }
    })
  }

  it('does not reload a panel that is closed', async () => {
    await addTaskFromMessage({ title: 'Boiler repair', account: 'acct1', threadId: 'acct1#thread#4' })
    expect(calls.some((call) => call.command === 'tasks.items')).toBe(false)
  })
})

describe('the Tasks toggle', () => {
  it('is off by default', () => {
    expect(settings$.tasksEnabled.peek()).toBe(false)
  })

  // Otherwise a panel stays on screen that the rail no longer offers a way to.
  it('hides the panel when switched off, remembering the list', async () => {
    setTasksEnabled(true)
    await openTasksPanel()
    expect(ui$.tasksPanelOpen.get()).toBe(true)

    setTasksEnabled(false)
    expect(ui$.tasksPanelOpen.get()).toBe(false)
    expect(ui$.activeTaskList.get()).toBe('list-1')
  })
})

describe('closeTasksPanel', () => {
  it('hides the panel and drops the open editor', async () => {
    await openTasksPanel()
    tasks$.editingId.set('task-1')
    closeTasksPanel()
    expect(ui$.tasksPanelOpen.get()).toBe(false)
    expect(tasks$.editingId.get()).toBe('')
  })
})

describe('undoable deletes', () => {
  // Tasks exist nowhere but this device, so a delete with no way back is the
  // one action here that can lose work outright.
  it('offers an undo that hands the core back what it removed', async () => {
    await openTasksPanel()
    await deleteTask('task-1')

    const undo = ui$.toastUndo.get() as (() => void) | null
    expect(typeof undo).toBe('function')
    calls.length = 0
    undo?.()
    await Promise.resolve()

    const restore = calls.find((call) => call.command === 'tasks.restore')
    expect(restore?.payload.restore).toEqual({ tasks: [task()] })
  })

  it('restores a deleted list along with its tasks', async () => {
    await openTasksPanel()
    await deleteTaskList('list-1')

    const undo = ui$.toastUndo.get() as (() => void) | null
    calls.length = 0
    undo?.()
    await Promise.resolve()

    const restore = calls.find((call) => call.command === 'tasks.restore')
    expect(restore?.payload.restore.list.id).toBe('list-1')
    expect(restore?.payload.restore.tasks).toHaveLength(1)
  })

  it('offers an undo after clearing completed tasks', async () => {
    await openTasksPanel()
    await clearCompletedTasks()
    expect(typeof ui$.toastUndo.get()).toBe('function')
  })

  // A delete the core could not describe leaves a plain toast rather than an
  // Undo button that would do nothing.
  it('shows a plain toast when there is nothing to restore', async () => {
    await openTasksPanel()
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      if (command === 'tasks.items') return { tasks: [] }
      if (command === 'tasks.delete') return { ok: true, restore: null }
      return { ok: true }
    }
    await deleteTask('task-1')
    expect(ui$.toast.get()).not.toBe('')
    expect(ui$.toastUndo.get()).toBe(null)
  })
})

describe('restoreTasksSession', () => {
  it('loads the lists and items when restoring an open panel', async () => {
    await restoreTasksSession({ tasks_enabled: true, session_tasks_panel: true, session_task_list: 'list-2' })
    expect(ui$.tasksPanelOpen.get()).toBe(true)
    expect(ui$.activeTaskList.get()).toBe('list-2')
    expect(tasks$.lists.get()).toHaveLength(2)
    expect(tasks$.items.get()).toHaveLength(1)
  })

  it('replaces a missing remembered list with the default', async () => {
    await restoreTasksSession({ tasks_enabled: true, session_tasks_panel: true, session_task_list: 'deleted' })
    expect(ui$.activeTaskList.get()).toBe('list-1')
    expect(tasks$.lists.get()).toHaveLength(2)
  })

  it('keeps the panel closed when Tasks is disabled', async () => {
    await restoreTasksSession({ tasks_enabled: false, session_tasks_panel: true, session_task_list: 'list-2' })
    expect(ui$.tasksPanelOpen.get()).toBe(false)
    expect(calls.some((call) => call.command.startsWith('tasks.'))).toBe(false)
  })
})
