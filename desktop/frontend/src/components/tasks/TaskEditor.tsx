import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { CalendarDays, X } from 'lucide-react'

import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

import { useTranslation } from '../../lib/i18n'
import { fromDateInputValue, toDateInputValue } from '../../lib/date'
import { tasks$, updateTask, type Task, type TaskList } from '../../states/tasks'

function measureNotes(textarea: HTMLTextAreaElement) {
  // Reset first so deleting text lets the field shrink to its one-row minimum.
  textarea.style.height = 'auto'
  textarea.style.height = `${textarea.scrollHeight}px`
}

/**
 * Inline fields for one task. Edits save on blur rather than behind a Save
 * button: a task is a scrap of text, and asking the user to confirm each scrap
 * is more ceremony than the content deserves.
 */
export function TaskEditor({ task, lists, actions }: { task: Task; lists: TaskList[]; actions?: ReactNode }) {
  const { t } = useTranslation()
  const [title, setTitle] = useState(task.title)
  const [notes, setNotes] = useState(task.notes)
  const notesRef = useRef<HTMLTextAreaElement>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  const [addingDueDate, setAddingDueDate] = useState(false)

  useLayoutEffect(() => {
    const textarea = notesRef.current
    if (textarea) measureNotes(textarea)
  }, [notes])

  useLayoutEffect(() => {
    const textarea = notesRef.current
    if (!textarea) return
    let width = textarea.clientWidth
    const observer = new ResizeObserver(() => {
      // Height changes come from measuring; only width changes need another pass.
      if (textarea.clientWidth === width) return
      width = textarea.clientWidth
      measureNotes(textarea)
    })
    observer.observe(textarea)
    return () => observer.disconnect()
  }, [])

  // Re-seed when the pane switches to a different task, so the fields don't
  // keep showing the previous one's text.
  useEffect(() => {
    setTitle(task.title)
    setNotes(task.notes)
    setAddingDueDate(false)
  }, [task.id])

  function finishEditing() {
    // Save the focused field before removing it from the row. Focus that has
    // already moved elsewhere stays where the user put it.
    const active = document.activeElement
    if (active instanceof HTMLElement && rootRef.current?.contains(active)) active.blur()
    tasks$.editingId.set('')
  }

  // Leaving the row is the same as Done. Watching clicks and focus moves rather
  // than the fields' own blur keeps the row open when the window is deactivated
  // and when WebKit declines to focus a clicked button.
  useEffect(() => {
    const isOutside = (target: EventTarget | null) => {
      const row = rootRef.current?.closest('[data-task-row]') ?? rootRef.current
      return target instanceof Node && !!row && !row.contains(target)
    }
    let pressing = false
    const onMouseDown = () => {
      pressing = true
    }
    const onMouseUp = () => {
      pressing = false
    }
    const onClick = (event: MouseEvent) => {
      if (isOutside(event.target)) finishEditing()
    }
    const onFocusIn = (event: FocusEvent) => {
      // A press closes on click instead: collapsing the row at mousedown would
      // move whatever the pointer is about to release on.
      if (!pressing && isOutside(event.target)) finishEditing()
    }
    document.addEventListener('mousedown', onMouseDown, true)
    document.addEventListener('mouseup', onMouseUp, true)
    document.addEventListener('click', onClick, true)
    document.addEventListener('focusin', onFocusIn)
    return () => {
      document.removeEventListener('mousedown', onMouseDown, true)
      document.removeEventListener('mouseup', onMouseUp, true)
      document.removeEventListener('click', onClick, true)
      document.removeEventListener('focusin', onFocusIn)
    }
  }, [])

  return (
    <div
      ref={rootRef}
      className="flex min-w-0 flex-col gap-1 py-0.5"
      onKeyDown={(event) => {
        if (event.key !== 'Escape') return
        event.stopPropagation()
        finishEditing()
      }}
    >
      <div className="flex min-w-0 items-start gap-1.5">
        <input
          autoFocus
          aria-label={t('tasks.titlePlaceholder')}
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          onBlur={() => {
            if (title.trim() && title !== task.title) void updateTask(task.id, { title })
            else setTitle(task.title)
          }}
          placeholder={t('tasks.titlePlaceholder')}
          className="min-w-0 flex-1 bg-transparent text-sm leading-5 text-primary outline-none"
        />
        {actions}
      </div>

      <textarea
        ref={notesRef}
        aria-label={t('tasks.notesPlaceholder')}
        value={notes}
        onChange={(event) => setNotes(event.target.value)}
        onBlur={() => {
          if (notes !== task.notes) void updateTask(task.id, { notes })
        }}
        placeholder={t('tasks.notesPlaceholder')}
        rows={1}
        className="max-h-32 w-full resize-none overflow-y-auto bg-transparent text-xs leading-4 text-secondary outline-none"
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
        <Button
          variant="ghost"
          size="sm"
          // Keep focus in the field until click: a blur can shrink the notes
          // textarea and move this button away before the pointer is released.
          onMouseDown={(event) => event.preventDefault()}
          onClick={finishEditing}
        >
          {t('buttons.done')}
        </Button>
      </div>
    </div>
  )
}
