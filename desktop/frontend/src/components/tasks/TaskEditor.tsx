import { useEffect, useState } from 'react'

import { useTranslation } from '../../lib/i18n'
import { fromDateInputValue, toDateInputValue } from '../../lib/date'
import { tasks$, updateTask, type Task, type TaskList } from '../../states/tasks'

/**
 * The detail pane for one task. Edits save on blur rather than behind a Save
 * button: a task is a scrap of text, and asking the user to confirm each scrap
 * is more ceremony than the content deserves.
 */
export function TaskEditor({ task, lists }: { task: Task; lists: TaskList[] }) {
  const { t } = useTranslation()
  const [title, setTitle] = useState(task.title)
  const [notes, setNotes] = useState(task.notes)

  // Re-seed when the pane switches to a different task, so the fields don't
  // keep showing the previous one's text.
  useEffect(() => {
    setTitle(task.title)
    setNotes(task.notes)
  }, [task.id])

  return (
    <div className="flex flex-col gap-2 border-t border-border px-3 pb-3 pt-2">
      <input
        value={title}
        onChange={(event) => setTitle(event.target.value)}
        onBlur={() => {
          if (title.trim() && title !== task.title) void updateTask(task.id, { title })
          else setTitle(task.title)
        }}
        placeholder={t('tasks.titlePlaceholder')}
        className="w-full rounded-lg bg-app px-2 py-1.5 text-sm text-primary outline-none ring-1 ring-border focus:ring-accent"
      />

      <textarea
        value={notes}
        onChange={(event) => setNotes(event.target.value)}
        onBlur={() => {
          if (notes !== task.notes) void updateTask(task.id, { notes })
        }}
        placeholder={t('tasks.notesPlaceholder')}
        rows={2}
        className="w-full resize-none rounded-lg bg-app px-2 py-1.5 text-sm text-primary outline-none ring-1 ring-border focus:ring-accent"
      />

      <label className="flex items-center gap-2 text-xs text-secondary">
        <span className="shrink-0">{t('tasks.dueDate')}</span>
        <input
          type="date"
          value={toDateInputValue(task.due_at)}
          onChange={(event) => void updateTask(task.id, { dueAt: fromDateInputValue(event.target.value) })}
          className="rounded-lg bg-app px-2 py-1 text-xs text-primary outline-none ring-1 ring-border focus:ring-accent"
        />
        {task.due_at > 0 ? (
          <button
            type="button"
            onClick={() => void updateTask(task.id, { dueAt: 0 })}
            className="text-xs text-secondary hover:text-primary"
          >
            {t('tasks.clearDueDate')}
          </button>
        ) : null}
      </label>

      {lists.length > 1 ? (
        <label className="flex items-center gap-2 text-xs text-secondary">
          <span className="shrink-0">{t('tasks.moveToList')}</span>
          <select
            value={task.list_id}
            onChange={(event) => {
              const next = event.target.value
              if (next === task.list_id) return
              // The task leaves this list, so the editor has nothing left to show.
              tasks$.editingId.set('')
              void updateTask(task.id, { listId: next })
            }}
            className="min-w-0 flex-1 rounded-lg bg-app px-2 py-1 text-xs text-primary outline-none ring-1 ring-border focus:ring-accent"
          >
            {lists.map((list) => (
              <option key={list.id} value={list.id}>
                {list.title}
              </option>
            ))}
          </select>
        </label>
      ) : null}
    </div>
  )
}
