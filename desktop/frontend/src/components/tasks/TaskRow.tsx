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
  renderEditor?: (actions: ReactNode) => ReactNode
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
  const overdue = taskIsOverdue(task.due_at) && !task.done
  const actions = (
    <div className="flex shrink-0 items-center gap-0.5" data-no-drag>
      {task.thread_id && onOpenMessage ? (
        <IconButton label={t('tasks.openMessage')} icon={Mail} size="sm" onClick={onOpenMessage} />
      ) : null}
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
        className={`group flex items-start gap-1.5 px-3 py-1.5 hover:bg-hover ${
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
            {renderEditor?.(actions)}
          </div>
        ) : (
          <button type="button" onClick={onOpen} className="min-w-0 flex-1 py-0.5 text-left">
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
            {task.due_at > 0 ? (
              <span
                className={`mt-1 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[0.6875rem] font-medium ${
                  overdue ? 'bg-rose-500/10 text-rose-600 dark:text-rose-400' : 'bg-raised text-secondary'
                }`}
              >
                <CalendarDays size={12} aria-hidden="true" />
                {formatDueDate(task.due_at)}
              </span>
            ) : null}
          </button>
        )}

        {expanded ? null : actions}
      </div>
    </div>
  )
}
