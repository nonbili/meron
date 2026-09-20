import { observable } from '@legendapp/state'

import { invoke } from '../lib/bridge'
import { t } from '../lib/i18n'
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
  /** Whether ticked tasks stay on screen under a "Completed" heading. */
  showCompleted: false,
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

/**
 * Restore the panel from the last session. The list id is not validated here —
 * the lists live in the core, not in settings, so this seeds the id and
 * [`openTasksPanel`] confirms it still exists when the panel opens.
 */
export function restoreTasksSession(prefs: Record<string, unknown>) {
  panelOpenSession.restore(prefs)
  activeListSession.restore(prefs)
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

async function loadTaskLists(): Promise<{ lists: TaskList[]; defaultListId: string }> {
  const res = await invoke<{ lists?: TaskList[]; default_list_id?: string }>('tasks.lists')
  const lists = res?.lists ?? []
  tasks$.lists.set(lists)
  return { lists, defaultListId: res?.default_list_id ?? '' }
}

/** Re-read one list's items. The core decides the order; we never re-sort. */
export async function loadTasks(listId = ui$.activeTaskList.peek()) {
  if (!listId) return
  tasks$.loading.set(true)
  try {
    const res = await invoke<{ tasks?: Task[] }>('tasks.items', {
      list_id: listId,
      include_completed: tasks$.showCompleted.peek(),
    })
    // A list switch that landed while this was in flight owns the panel now.
    if (ui$.activeTaskList.peek() !== listId) return
    tasks$.items.set(res?.tasks ?? [])
  } finally {
    tasks$.loading.set(false)
  }
}

export async function setShowCompleted(show: boolean) {
  tasks$.showCompleted.set(show)
  await loadTasks()
}

export async function addTask(title: string) {
  const listId = ui$.activeTaskList.peek()
  if (!listId || !title.trim()) return
  await invoke('tasks.create', { list_id: listId, title: title.trim() })
  await loadTasks(listId)
}

/**
 * Turn a message into a task. Used from the mail surfaces, where there is no
 * list picker, so the core files it under the default list.
 */
export async function addTaskFromMessage(input: {
  title: string
  account: string
  threadId: string
  messageId?: string
}) {
  await invoke('tasks.create', {
    list_id: '',
    title: input.title,
    account: input.account,
    thread_id: input.threadId,
    message_id: input.messageId ?? '',
  })
  showToast(t('tasks.addedFromMessage'))
  // The panel is very likely open and showing the list this landed in, so it
  // has to repaint; when it is closed the next open reads fresh anyway.
  if (ui$.tasksPanelOpen.peek()) await loadTasks()
}

export async function setTaskDone(taskId: string, done: boolean) {
  await invoke('tasks.setDone', { task_id: taskId, done })
  await loadTasks()
}

export async function updateTask(
  taskId: string,
  patch: { title?: string; notes?: string; dueAt?: number; listId?: string },
) {
  const payload: Record<string, unknown> = { task_id: taskId }
  if (patch.title !== undefined) payload.title = patch.title
  if (patch.notes !== undefined) payload.notes = patch.notes
  if (patch.dueAt !== undefined) payload.due_at = patch.dueAt
  if (patch.listId !== undefined) payload.list_id = patch.listId
  await invoke('tasks.update', payload)
  await loadTasks()
}

export async function deleteTask(taskId: string) {
  const res = await invoke<{ restore?: unknown }>('tasks.delete', { task_id: taskId })
  if (tasks$.editingId.peek() === taskId) tasks$.editingId.set('')
  await loadTasks()
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
    showToast(message)
    return
  }
  showUndoToast(message, () => {
    void invoke('tasks.restore', { restore }).then(async () => {
      await refreshTaskLists()
      await loadTasks()
    })
  })
}

async function refreshTaskLists() {
  await loadTaskLists()
}

/** Persist the order the user just dragged the rows into. */
export async function reorderTasks(orderedIds: string[]) {
  const listId = ui$.activeTaskList.peek()
  if (!listId) return
  await invoke('tasks.reorder', { list_id: listId, task_ids: orderedIds })
  await loadTasks(listId)
}

export async function clearCompletedTasks() {
  const listId = ui$.activeTaskList.peek()
  if (!listId) return
  const res = await invoke<{ removed?: number; restore?: unknown }>('tasks.clearCompleted', {
    list_id: listId,
  })
  await loadTasks(listId)
  if (res?.removed) offerUndo(t('tasks.completedCleared'), res.restore)
}

export async function createTaskList(title: string) {
  const res = await invoke<{ list?: TaskList }>('tasks.listCreate', { title })
  await loadTaskLists()
  const id = res?.list?.id
  if (id) {
    ui$.activeTaskList.set(id)
    await loadTasks(id)
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
  if (next) await loadTasks(next)
  else tasks$.items.set([])
  offerUndo(t('tasks.listDeleted', { name: list.title }), res?.restore)
}
