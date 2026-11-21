import { beforeEach, describe, expect, it } from 'bun:test'

import { ui$ } from './ui'
import { settings$, setTasksEnabled } from './settings'
import { compose$ } from './composeState'
import {
  addTaskFromMessage,
  clearCompletedTasks,
  deleteTask,
  deleteTaskList,
  closeTasksPanel,
  loadTasks,
  openTaskMail,
  openTasksPanel,
  restoreTasksSession,
  taskListGroups,
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
let threadMessages: any[] = []
let threadTasks: Task[] = []

beforeEach(() => {
  calls.length = 0
  items = [task()]
  threadTasks = []
  ui$.tasksPanelOpen.set(false)
  ui$.activeTaskList.set('')
  ui$.toast.set('')
  ui$.toastUndo.set(null)
  tasks$.lists.set([])
  tasks$.items.set([])
  compose$.tabs.set([])
  compose$.activeTab.set('')
  threadMessages = [
    {
      id: 'm1',
      account_id: 'acct',
      folder_id: 'INBOX',
      thread_id: 'acct#INBOX#t.abc',
      from_name: 'Landlord',
      from_addr: 'landlord@example.com',
      subject: 'Rent',
      date: 1,
    },
  ]
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
          if (command === 'tasks.forThread') return { tasks: threadTasks }
          if (command === 'mail.threadRead') return { messages: threadMessages }
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
  // Completed tasks always come along: they sit under a collapsible heading
  // at the bottom, as in Google Tasks, rather than behind a toggle.
  it('asks for completed tasks too', async () => {
    ui$.activeTaskList.set('list-1')
    await loadTasks()
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

  it('says so instead of adding a thread that already has an open task', async () => {
    threadTasks = [task({ thread_id: 'acct1#thread#4' })]
    await addTaskFromMessage({ title: 'Boiler repair', account: 'acct1', threadId: 'acct1#thread#4' })
    expect(calls.find((call) => call.command === 'tasks.forThread')?.payload).toEqual({ thread_id: 'acct1#thread#4' })
    expect(calls.some((call) => call.command === 'tasks.create')).toBe(false)
    expect(ui$.toast.get()).toBe('Already in Tasks')
  })

  it('files a double press once', async () => {
    const input = { title: 'Boiler repair', account: 'acct1', threadId: 'acct1#thread#4' }
    await Promise.all([addTaskFromMessage(input), addTaskFromMessage(input)])
    expect(calls.filter((call) => call.command === 'tasks.create')).toHaveLength(1)
  })

  it('trusts a caller that already checked', async () => {
    threadTasks = [task({ thread_id: 'acct1#thread#4' })]
    await addTaskFromMessage({ title: 'Boiler repair', account: 'acct1', threadId: 'acct1#thread#4', checked: true })
    expect(calls.some((call) => call.command === 'tasks.forThread')).toBe(false)
    expect(calls.some((call) => call.command === 'tasks.create')).toBe(true)
  })

  it('adds again once the earlier task is done', async () => {
    threadTasks = [task({ thread_id: 'acct1#thread#4', done: true })]
    await addTaskFromMessage({ title: 'Boiler repair', account: 'acct1', threadId: 'acct1#thread#4' })
    expect(calls.some((call) => call.command === 'tasks.create')).toBe(true)
  })
})

describe('the Tasks toggle', () => {
  it('is on by default', () => {
    expect(settings$.tasksEnabled.peek()).toBe(true)
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

  it('reports an undo the core could not restore', async () => {
    await openTasksPanel()
    await deleteTask('task-1')
    const undo = ui$.toastUndo.get() as (() => void) | null
    const invokeTask = (window as any).go.main.App.Invoke
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      if (command === 'tasks.restore') throw new Error('restore refused')
      return invokeTask(command, payload)
    }

    undo?.()
    await new Promise((resolve) => setTimeout(resolve, 0))

    expect(ui$.toast.get()).toBe('restore refused')
    expect(ui$.toastTone.get()).toBe('error')
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

  it('treats Tasks as enabled when the pref was never set', async () => {
    await restoreTasksSession({ session_tasks_panel: true, session_task_list: 'list-2' })
    expect(ui$.tasksPanelOpen.get()).toBe(true)
  })

  it('keeps the panel closed when Tasks is disabled', async () => {
    await restoreTasksSession({ tasks_enabled: false, session_tasks_panel: true, session_task_list: 'list-2' })
    expect(ui$.tasksPanelOpen.get()).toBe(false)
    expect(calls.some((call) => call.command.startsWith('tasks.'))).toBe(false)
  })
})

describe('openTaskMail', () => {
  const threadId = 'acct#INBOX#t.abc'

  it('opens the mail in its own tab', async () => {
    await openTaskMail(threadId)
    expect(compose$.tabs.get().map((tab) => tab.threadId)).toEqual([threadId])
  })

  // The mail was deleted: say so in the task's own words rather than leaving
  // the click unexplained. The link keeps working — a later sync can bring the
  // conversation back, and the next press then opens it.
  it('reports mail the read cannot find', async () => {
    threadMessages = []

    await openTaskMail(threadId)

    expect(compose$.tabs.get()).toEqual([])
    expect(ui$.toast.get()).toBe('Message no longer available')
  })

  it('opens a thread that comes back', async () => {
    threadMessages = []
    await openTaskMail(threadId)
    threadMessages = [
      {
        id: 'm1',
        account_id: 'acct',
        folder_id: 'INBOX',
        thread_id: threadId,
        from_name: 'Landlord',
        from_addr: 'landlord@example.com',
        subject: 'Rent',
        date: 1,
      },
    ]

    await openTaskMail(threadId)

    expect(compose$.tabs.get().map((tab) => tab.threadId)).toEqual([threadId])
  })
})

describe('task loading recovery', () => {
  it('shares concurrent reads and preserves items until a timeout retry succeeds', async () => {
    ui$.activeTaskList.set('list-1')
    const previous = task({ id: 'previous' })
    tasks$.items.set([previous])
    let attempts = 0
    let release!: (value: unknown) => void
    const response = new Promise((resolve) => {
      release = resolve
    })
    ;(window as any).go.main.App.Invoke = async () => {
      if (++attempts === 1) throw new Error('sidecar tasks.items timeout after 5s')
      return response
    }
    const first = loadTasks()
    const second = loadTasks()
    expect(first).toBe(second)
    await Promise.resolve()
    await Promise.resolve()
    expect(tasks$.items.peek()[0].id).toBe('previous')
    expect(tasks$.loading.peek()).toBe(true)
    release({ tasks: [task({ id: 'recovered' })] })
    await first
    expect(attempts).toBe(2)
    expect(tasks$.items.peek()[0].id).toBe('recovered')
    expect(tasks$.loading.peek()).toBe(false)
  })

  it('bounds retries and does not retry core errors', async () => {
    ui$.activeTaskList.set('list-1')
    let attempts = 0
    ;(window as any).go.main.App.Invoke = async () => {
      attempts++
      throw new Error('sidecar tasks.items timeout after 5s')
    }
    await expect(loadTasks()).rejects.toThrow('timeout')
    expect(attempts).toBe(2)
    expect(tasks$.loading.peek()).toBe(false)
    attempts = 0
    ;(window as any).go.main.App.Invoke = async () => {
      attempts++
      throw new Error('sidecar tasks.items error: missing list')
    }
    await expect(loadTasks()).rejects.toThrow('missing list')
    expect(attempts).toBe(1)
  })

  it('keeps the selected list loading when an older list finishes', async () => {
    const releases = new Map<string, (value: unknown) => void>()
    ;(window as any).go.main.App.Invoke = (_command: string, payload: any) =>
      new Promise((resolve) => releases.set(payload.list_id, resolve))
    ui$.activeTaskList.set('list-1')
    const first = loadTasks()
    ui$.activeTaskList.set('list-2')
    const second = loadTasks()
    releases.get('list-1')!({ tasks: [task()] })
    await first
    expect(tasks$.loading.peek()).toBe(true)
    expect(tasks$.items.peek()).toEqual([])
    releases.get('list-2')!({ tasks: [task({ list_id: 'list-2' })] })
    await second
    expect(tasks$.items.peek()[0].list_id).toBe('list-2')
    expect(tasks$.loading.peek()).toBe(false)
  })
})

it('a fresh read after a mutation supersedes an older in-flight read', async () => {
  ui$.activeTaskList.set('list-1')
  const releases: ((value: unknown) => void)[] = []
  ;(window as any).go.main.App.Invoke = () => new Promise((resolve) => releases.push(resolve))
  const old = loadTasks()
  const fresh = loadTasks('list-1', true)
  releases[1]({ tasks: [task({ id: 'new' })] })
  await fresh
  releases[0]({ tasks: [task({ id: 'old' })] })
  await old
  expect(tasks$.items.peek()[0].id).toBe('new')
})

it('retries a timed-out list read before loading the restored panel', async () => {
  const invoke = (window as any).go.main.App.Invoke
  let listAttempts = 0
  ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
    if (command === 'tasks.lists' && ++listAttempts === 1) {
      throw new Error('sidecar tasks.lists timeout after 5s')
    }
    return invoke(command, payload)
  }
  await restoreTasksSession({ session_tasks_panel: true, session_task_list: 'list-1' })
  expect(listAttempts).toBe(2)
  expect(tasks$.items.peek()[0].id).toBe('task-1')
})

it('does not resurrect an older snapshot when a post-mutation read fails', async () => {
  ui$.activeTaskList.set('list-1')
  tasks$.items.set([task({ id: 'current' })])
  let release!: (value: unknown) => void
  const response = new Promise((resolve) => {
    release = resolve
  })
  let requests = 0
  ;(window as any).go.main.App.Invoke = async () => {
    if (++requests === 1) return response
    throw new Error('sidecar tasks.items timeout after 5s')
  }
  const old = loadTasks()
  await expect(loadTasks('list-1', true)).rejects.toThrow('timeout')
  release({ tasks: [task({ id: 'deleted-by-mutation' })] })
  await old
  expect(tasks$.items.peek()[0].id).toBe('current')
  expect(tasks$.loading.peek()).toBe(false)
})

describe('taskListGroups', () => {
  it('keeps local lists first and tells same-named lists of two accounts apart', () => {
    const groups = taskListGroups([
      { id: 'g1', title: 'My Tasks', provider: 'google', account: 'sam@example.test' },
      { id: 'local', title: 'My Tasks' },
      { id: 'c1', title: 'Work', provider: 'caldav', account: 'sam@example.test' },
      { id: 'g2', title: 'My Tasks', provider: 'google', account: 'alex@example.test' },
      { id: 'g3', title: 'Errands', provider: 'google', account: 'sam@example.test' },
    ])
    expect(groups.map(({ label, lists }) => [label, lists.map(({ id }) => id)])).toEqual([
      ['', ['local']],
      ['sam@example.test · Google Tasks', ['g1', 'g3']],
      ['sam@example.test · CalDAV', ['c1']],
      ['alex@example.test · Google Tasks', ['g2']],
    ])
  })

  it('names the servers of two CalDAV accounts that share a username', () => {
    const groups = taskListGroups([
      { id: 'a', title: 'Tasks', provider: 'caldav', account: 'sam', account_id: '1', server: 'https://dav.one.test/' },
      { id: 'b', title: 'Tasks', provider: 'caldav', account: 'sam', account_id: '2', server: 'https://dav.two.test/' },
    ])
    expect(groups.map(({ label }) => label)).toEqual(['sam · CalDAV (dav.one.test)', 'sam · CalDAV (dav.two.test)'])
  })
})
