import { useEffect, useState } from 'react'
import { CalendarDays, X } from 'lucide-react'

import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

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
  const [addingDueDate, setAddingDueDate] = useState(false)

  // Re-seed when the pane switches to a different task, so the fields don't
  // keep showing the previous one's text.
  useEffect(() => {
    setTitle(task.title)
    setNotes(task.notes)
    setAddingDueDate(false)
  }, [task.id])

  return (
    <div
      className="flex flex-col gap-2 border-t border-border px-3 pb-3 pt-2"
      onKeyDown={(event) => {
        if (event.key !== 'Escape') return
        event.stopPropagation()
        // Flush the focused field's blur save before unmounting the editor.
        if (event.target instanceof HTMLElement) event.target.blur()
        tasks$.editingId.set('')
      }}
    >
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

      <div className="flex items-center justify-between gap-2 pt-1">
        {task.due_at > 0 || addingDueDate ? (
          <div className="flex min-w-0 items-center gap-1 rounded-lg bg-raised px-2 ring-1 ring-border focus-within:ring-accent">
            <CalendarDays size={14} className="shrink-0 text-secondary" aria-hidden="true" />
            <input
              autoFocus={addingDueDate}
              type="date"
              aria-label={t('tasks.dueDate')}
              value={toDateInputValue(task.due_at)}
              onChange={(event) => void updateTask(task.id, { dueAt: fromDateInputValue(event.target.value) })}
              className="min-w-0 bg-transparent py-1.5 text-xs text-primary outline-none"
            />
            <IconButton
              label={t('tasks.clearDueDate')}
              icon={X}
              size="sm"
              onClick={() => {
                setAddingDueDate(false)
                if (task.due_at > 0) void updateTask(task.id, { dueAt: 0 })
              }}
            />
          </div>
        ) : (
          <IconButton label={t('tasks.dueDate')} icon={CalendarDays} size="md" onClick={() => setAddingDueDate(true)} />
        )}
        <Button variant="secondary" size="sm" onClick={() => tasks$.editingId.set('')}>
          {t('buttons.done')}
        </Button>
      </div>
    </div>
  )
}
