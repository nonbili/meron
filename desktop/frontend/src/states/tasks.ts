import { observable } from '@legendapp/state'

import { invoke, invokeRetryingTimeout } from '../lib/bridge'
import { t } from '../lib/i18n'
import { openThreadTabById } from './compose'
import { showToast, showUndoToast, ui$ } from './ui'
import { persistedField } from '../lib/sessionPref'

export type TaskList = {
  id: string
  title: string
}

export type Task = {
  id: string
  list_id: string
  title: string
  notes: string
  /** Epoch seconds, 0 when the task has no due date. */
  due_has_time?: boolean
  due_at: number
  completed_at: number
  done: boolean
  /** Set when the task came from a message; all three empty otherwise. */
  account: string
  thread_id: string
  message_id: string
}

/**
 * Tasks runtime state. Unlike kanban boards — which the clients each parse out
 * of a settings blob — the lists and their items live in the core's tables and
 * are only cached here, so there is one definition of what a task is.
 *
 * Whether the panel is showing, and which list it shows, live on `ui$` — the
 * panel is app chrome that sits beside mail or kanban rather than a view that
 * replaces one, so nothing about opening a board or a mailbox disturbs it.
 */
export const tasks$ = observable({
  lists: [] as TaskList[],
  items: [] as Task[],
  loading: false,
  /** The task whose editor is open, '' for none. */
  editingId: '',
})

// The panel state survives a restart, like the selected board and folder do:
// a panel you left open is part of how you had the app arranged.
const panelOpenSession = persistedField(ui$.tasksPanelOpen, 'session_tasks_panel', (raw) =>
  typeof raw === 'boolean' ? raw : undefined,
)
const activeListSession = persistedField(ui$.activeTaskList, 'session_task_list', (raw) =>
  typeof raw === 'string' ? raw : undefined,
)

/** Prefs keys this module owns; boot requests them in its single prefsGet. */
export const TASKS_SESSION_KEYS = [panelOpenSession.key, activeListSession.key]

/** Restore and validate the remembered list before displaying its panel. */
export async function restoreTasksSession(prefs: Record<string, unknown>) {
  activeListSession.restore(prefs)
  panelOpenSession.restore(prefs.tasks_enabled !== false ? prefs : { ...prefs, session_tasks_panel: false })
  if (ui$.tasksPanelOpen.peek()) await openTasksPanel()
}

/**
 * Show the Tasks panel, on `listId` or on whichever list the core defaults to.
 *
 * Opening it disturbs nothing else: the mail or kanban view keeps its place,
 * which is the point of a panel — you work a list against the thread list
 * rather than instead of it.
 */
export async function openTasksPanel(listId = '') {
  ui$.tasksPanelOpen.set(true)
  const lists = await loadTaskLists()
  const target =
    lists.lists.find((list) => list.id === listId)?.id ??
    lists.lists.find((list) => list.id === ui$.activeTaskList.peek())?.id ??
    lists.defaultListId
  if (!target) return
  ui$.activeTaskList.set(target)
  await loadTasks(target)
}

/** Hide the panel. The list it was on is remembered for the next open. */
export function closeTasksPanel() {
  ui$.tasksPanelOpen.set(false)
  tasks$.editingId.set('')
}

/** What the rail button does: show the panel, or hide it if it is showing. */
export function toggleTasksPanel() {
  if (ui$.tasksPanelOpen.peek()) closeTasksPanel()
  else void openTasksPanel()
}

export async function loadTaskLists(): Promise<{ lists: TaskList[]; defaultListId: string }> {
  const res = await invokeRetryingTimeout<{ lists?: TaskList[]; default_list_id?: string }>('tasks.lists', {}, 2)
  const lists = res?.lists ?? []
  tasks$.lists.set(lists)
  return { lists, defaultListId: res?.default_list_id ?? '' }
}

// Share concurrent reads of the same list (session restore and panel opening
// can overlap). A later list selection still owns what is displayed.
const taskLoads = new Map<string, Promise<void>>()

/** Re-read one list's items. Mutations request a fresh read so an older
 * in-flight snapshot cannot hide the change. */
export function loadTasks(listId = ui$.activeTaskList.peek(), fresh = false): Promise<void> {
  if (!listId) return Promise.resolve()
  const existing = taskLoads.get(listId)
  if (existing && !fresh) return existing
  tasks$.loading.set(true)
  let pending!: Promise<void>
  pending = (async () => {
    try {
      const res = await invokeRetryingTimeout<{ tasks?: Task[] }>(
        'tasks.items',
        {
          list_id: listId,
          include_completed: true,
        },
        2,
      )
      if (ui$.activeTaskList.peek() !== listId || taskLoads.get(listId) !== pending) return
      tasks$.items.set(res?.tasks ?? [])
    } finally {
      if (taskLoads.get(listId) === pending) taskLoads.delete(listId)
      tasks$.loading.set(taskLoads.has(ui$.activeTaskList.peek()))
    }
  })()
  taskLoads.set(listId, pending)
  return pending
}

export async function addTask(title: string) {
  const listId = ui$.activeTaskList.peek()
  if (!listId || !title.trim()) return
  await invoke('tasks.create', { list_id: listId, title: title.trim() })
  await loadTasks(listId, true)
}

/**
 * Whether a thread already has a task that isn't done. A finished one doesn't
 * count: filing the same mail again after closing its task is a new follow-up.
 */
export async function threadHasOpenTask(threadId: string): Promise<boolean> {
  const res = await invoke<{ tasks?: Task[] }>('tasks.forThread', { thread_id: threadId })
  return (res?.tasks ?? []).some((task) => !task.done)
}

/** Says so, rather than adding it twice, when the thread is already in Tasks. */
export async function warnIfThreadInTasks(threadId: string): Promise<boolean> {
  const exists = await threadHasOpenTask(threadId).catch(() => false)
  if (exists) showToast(t('tasks.alreadyInTasks'))
  return exists
}

// Threads being filed right now, so a second press landing before the first
// has created its task isn't let through by a check that ran too early.
const filingThreads = new Set<string>()

/**
 * Turn a message into a task in the chosen list. Callers without an explicit
 * destination use the remembered selection, or the core default on first use.
 * `checked` is for a caller that already ran warnIfThreadInTasks itself.
 */
export async function addTaskFromMessage(input: {
  title: string
  account: string
  threadId: string
  messageId?: string
  listId?: string
  checked?: boolean
}) {
  if (filingThreads.has(input.threadId)) return
  filingThreads.add(input.threadId)
  try {
    if (input.threadId && !input.checked && (await warnIfThreadInTasks(input.threadId))) return
    await invoke('tasks.create', {
      list_id: input.listId ?? ui$.activeTaskList.peek(),
      title: input.title,
      account: input.account,
      thread_id: input.threadId,
      message_id: input.messageId ?? '',
    })
  } finally {
    filingThreads.delete(input.threadId)
  }
  showToast(t('tasks.addedFromMessage'))
  // The panel is very likely open and showing the list this landed in, so it
  // has to repaint; when it is closed the next open reads fresh anyway.
  if (ui$.tasksPanelOpen.peek()) await loadTasks(ui$.activeTaskList.peek(), true)
}

/**
 * Open the mail a task was made from. The read resolves the thread across the
 * account's folders, so a message that was merely moved (archived, trashed)
 * still opens; one that is gone for good says so in a toast of its own rather
 * than the notification wording.
 *
 * Nothing is remembered about a read that found nothing: the mail can come back
 * with the next sync, and a link the user can press again — and be told again —
 * beats one this app has quietly decided is dead.
 */
export async function openTaskMail(threadId: string) {
  if (!threadId) return
  await openThreadTabById(threadId, t('tasks.mailNotFound'))
}

export async function setTaskDone(taskId: string, done: boolean) {
  await invoke('tasks.setDone', { task_id: taskId, done })
  await loadTasks(ui$.activeTaskList.peek(), true)
}

export async function updateTask(
  taskId: string,
  patch: { title?: string; notes?: string; dueAt?: number; dueHasTime?: boolean; listId?: string },
) {
  const payload: Record<string, unknown> = { task_id: taskId }
  if (patch.title !== undefined) payload.title = patch.title
  if (patch.notes !== undefined) payload.notes = patch.notes
  if (patch.dueHasTime !== undefined) payload.due_has_time = patch.dueHasTime
  if (patch.dueAt !== undefined) payload.due_at = patch.dueAt
  if (patch.listId !== undefined) payload.list_id = patch.listId
  await invoke('tasks.update', payload)
  await loadTasks(ui$.activeTaskList.peek(), true)
}

export async function deleteTask(taskId: string) {
  const res = await invoke<{ restore?: unknown }>('tasks.delete', { task_id: taskId })
  if (tasks$.editingId.peek() === taskId) tasks$.editingId.set('')
  await loadTasks(ui$.activeTaskList.peek(), true)
  offerUndo(t('tasks.taskDeleted'), res?.restore)
}

/**
 * Pair a toast with an Undo that hands the core back what it just removed.
 *
 * Tasks exist nowhere but this device, so a delete with no way back is the one
 * action here that can lose work outright. The restore blob is opaque to us —
 * whatever the delete returned goes back verbatim.
 */
function offerUndo(message: string, restore: unknown) {
  if (!restore) {
    showToast(message, 'success', undefined, 'tasks')
    return
  }
  showUndoToast(
    message,
    () => {
      void invoke('tasks.restore', { restore })
        .then(async () => {
          await refreshTaskLists()
          await loadTasks(ui$.activeTaskList.peek(), true)
        })
        .catch((error) => {
          showToast(error instanceof Error ? error.message : t('notification.undoFailed'), 'error', undefined, 'tasks')
        })
    },
    'tasks',
  )
}

async function refreshTaskLists() {
  await loadTaskLists()
}

/** Persist the order the user just dragged the rows into. */
export async function reorderTasks(orderedIds: string[]) {
  const listId = ui$.activeTaskList.peek()
  if (!listId) return
  await invoke('tasks.reorder', { list_id: listId, task_ids: orderedIds })
  await loadTasks(listId, true)
}

export async function clearCompletedTasks() {
  const listId = ui$.activeTaskList.peek()
  if (!listId) return
  const res = await invoke<{ removed?: number; restore?: unknown }>('tasks.clearCompleted', {
    list_id: listId,
  })
  await loadTasks(listId, true)
  if (res?.removed) offerUndo(t('tasks.completedCleared'), res.restore)
}

export async function createTaskList(title: string) {
  const res = await invoke<{ list?: TaskList }>('tasks.listCreate', { title })
  await loadTaskLists()
  const id = res?.list?.id
  if (id) {
    ui$.activeTaskList.set(id)
    await loadTasks(id, true)
  }
}

export async function renameTaskList(listId: string, title: string) {
  if (!title.trim()) return
  await invoke('tasks.listRename', { list_id: listId, title: title.trim() })
  await loadTaskLists()
}

/**
 * Delete a list and everything in it.
 *
 * No confirmation dialog: the Undo in the toast is the better answer, because
 * it costs nothing on the common path and still recovers the whole list —
 * tasks, order and completion included.
 */
export async function deleteTaskList(listId: string) {
  const list = tasks$.lists.peek().find((entry) => entry.id === listId)
  if (!list) return
  const res = await invoke<{ restore?: unknown }>('tasks.listDelete', { list_id: listId })
  const remaining = (await loadTaskLists()).lists
  const next = remaining[0]?.id ?? ''
  ui$.activeTaskList.set(next)
  if (next) await loadTasks(next, true)
  else tasks$.items.set([])
  offerUndo(t('tasks.listDeleted', { name: list.title }), res?.restore)
}
