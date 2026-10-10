import { useState } from 'react'
import type { ReactNode } from 'react'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { CalendarDays, Mail, Trash2 } from 'lucide-react'

import { useTranslation } from '../../lib/i18n'
import { formatDueDate, taskIsOverdue } from '../../lib/date'
import { Checkbox } from '../field/Checkbox'
import { IconButton } from '../button/IconButton'
import type { Task } from '../../states/tasks'

interface TaskRowProps {
  task: Task
  /** Completed rows are not draggable: their order is by when they were ticked. */
  sortable: boolean
  /** Whether this row's editor replaces the title and notes. */
  expanded: boolean
  onToggle: (done: boolean) => void
  onOpen: () => void
  onDelete: () => void
  onOpenMessage?: () => void
  /** `focusDueDate` is set when the row was opened from its due date chip. */
  renderEditor?: (actions: ReactNode, focusDueDate: boolean) => ReactNode
}

export function TaskRow({
  task,
  sortable,
  expanded,
  onToggle,
  onOpen,
  onDelete,
  onOpenMessage,
  renderEditor,
}: TaskRowProps) {
  const { t } = useTranslation()
  const { listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: task.id,
    disabled: !sortable,
  })
  const [focusDueDate, setFocusDueDate] = useState(false)
  const overdue = taskIsOverdue(task.due_at) && !task.done
  const hasMessage = Boolean(task.thread_id && onOpenMessage)
  // The collapsed row lays these out itself; the editor takes them as a cluster.
  const actions = (
    <div className="flex shrink-0 items-center gap-0.5" data-no-drag>
      {hasMessage ? <IconButton label={t('tasks.openMessage')} icon={Mail} size="sm" onClick={onOpenMessage} /> : null}
      <IconButton
        label={t('tasks.deleteTask')}
        icon={Trash2}
        size="sm"
        variant="danger"
        onClick={onDelete}
        className="opacity-0 transition-opacity group-hover:opacity-100 focus:opacity-100"
      />
    </div>
  )

  return (
    <div
      ref={setNodeRef}
      data-task-row
      style={{
        transform: CSS.Translate.toString(transform),
        transition: isDragging ? 'none' : transition,
        zIndex: isDragging ? 10 : undefined,
        opacity: isDragging ? 0.5 : undefined,
      }}
      className={expanded ? 'bg-raised/60' : undefined}
    >
      <div
        className={`group relative flex items-start gap-1.5 px-3 py-1.5 hover:bg-hover ${
          sortable ? 'cursor-grab touch-manipulation active:cursor-grabbing' : ''
        }`}
        {...listeners}
      >
        {/* Keep controls out of the row's drag gesture. The mouse sensor's
          distance threshold preserves title clicks and suppresses clicks after a drag. */}
        <span className="flex h-6 shrink-0 items-center" data-no-drag>
          <Checkbox checked={task.done} onChange={(event) => onToggle(event.target.checked)} aria-label={task.title} />
        </span>

        {expanded ? (
          <div className="min-w-0 flex-1" data-no-drag>
            {renderEditor?.(actions, focusDueDate)}
          </div>
        ) : (
          <div className="min-w-0 flex-1">
            <button
              type="button"
              onClick={() => {
                setFocusDueDate(false)
                onOpen()
              }}
              className="block w-full py-0.5 text-left"
            >
              <span
                className={`line-clamp-2 break-words text-sm leading-5 ${
                  task.done ? 'text-secondary line-through' : 'text-primary'
                }`}
              >
                {task.title}
              </span>
              {task.notes ? (
                <span className="mt-0.5 line-clamp-4 whitespace-pre-wrap break-words text-xs text-secondary">
                  {task.notes}
                </span>
              ) : null}
            </button>
            {/* The message link is a button of its own, so the chips sit beside
              the title button rather than inside it. */}
            {task.due_at > 0 || hasMessage ? (
              <div className="mb-0.5 mt-0.5 flex flex-wrap items-center gap-1">
                {task.due_at > 0 ? (
                  <button
                    type="button"
                    onClick={() => {
                      setFocusDueDate(true)
                      onOpen()
                    }}
                    className={`inline-flex cursor-pointer items-center gap-1 rounded px-1.5 py-0.5 text-[0.6875rem] font-medium transition-colors ${
                      overdue
                        ? 'bg-rose-500/10 text-rose-600 hover:bg-rose-500/20 dark:text-rose-400'
                        : 'bg-raised text-secondary hover:bg-active hover:text-primary'
                    }`}
                  >
                    <CalendarDays size={12} aria-hidden="true" />
                    {formatDueDate(task.due_at)}
                  </button>
                ) : null}
                {hasMessage ? (
                  <button
                    type="button"
                    title={t('tasks.openMessage')}
                    aria-label={t('tasks.openMessage')}
                    onClick={onOpenMessage}
                    data-no-drag
                    className="inline-flex h-5 cursor-pointer items-center rounded bg-raised px-1.5 text-secondary transition-colors hover:bg-active hover:text-primary"
                  >
                    <Mail size={12} aria-hidden="true" />
                  </button>
                ) : null}
              </div>
            ) : null}
          </div>
        )}

        {/* Delete floats over the end of the title instead of reserving a column for it. */}
        {expanded ? null : (
          <div
            className="pointer-events-none absolute right-2 top-1 rounded-full bg-chats opacity-0 shadow-sm transition-opacity focus-within:pointer-events-auto focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100"
            data-no-drag
          >
            <IconButton label={t('tasks.deleteTask')} icon={Trash2} size="sm" variant="danger" onClick={onDelete} />
          </div>
        )}
      </div>
    </div>
  )
}
