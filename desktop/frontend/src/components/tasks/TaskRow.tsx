import type { ReactNode } from 'react'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { GripVertical, Mail, Trash2 } from 'lucide-react'

import { useTranslation } from '../../lib/i18n'
import { formatDueDate } from '../../lib/date'
import { Checkbox } from '../field/Checkbox'
import { IconButton } from '../button/IconButton'
import type { Task } from '../../states/tasks'

/** Whether a due date has already passed, so it can be called out. */
function isOverdue(dueAt: number): boolean {
  return dueAt > 0 && dueAt * 1000 < Date.now()
}

interface TaskRowProps {
  task: Task
  /** Completed rows are not draggable: their order is by when they were ticked. */
  sortable: boolean
  /** Whether this row's editor is open, showing `children` beneath it. */
  expanded: boolean
  onToggle: (done: boolean) => void
  onOpen: () => void
  onDelete: () => void
  onOpenMessage?: () => void
  children?: ReactNode
}

export function TaskRow({
  task,
  sortable,
  expanded,
  onToggle,
  onOpen,
  onDelete,
  onOpenMessage,
  children,
}: TaskRowProps) {
  const { t } = useTranslation()
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: task.id,
    disabled: !sortable,
  })
  const overdue = isOverdue(task.due_at) && !task.done

  return (
    <div
      ref={setNodeRef}
      style={{
        transform: CSS.Translate.toString(transform),
        transition: isDragging ? 'none' : transition,
        zIndex: isDragging ? 10 : undefined,
        opacity: isDragging ? 0.5 : undefined,
      }}
      className={expanded ? 'bg-raised/60' : undefined}
    >
      <div className="group flex items-start gap-1.5 px-1.5 py-1.5 hover:bg-hover">
        {/* The handle keeps to a fixed-width slot so titles stay aligned
          whether or not a row can be dragged. */}
        <span className="flex h-5 w-4 shrink-0 items-center justify-center">
          {sortable ? (
            <button
              type="button"
              // Drag lives on its own handle rather than the whole row: the row
              // opens the editor, and a row that both drags and opens does
              // neither reliably.
              className="cursor-grab text-secondary opacity-0 transition-opacity group-hover:opacity-100"
              aria-label={t('tasks.moveUp')}
              {...attributes}
              {...listeners}
            >
              <GripVertical size={13} />
            </button>
          ) : null}
        </span>

        <Checkbox
          className="mt-0.5 shrink-0"
          checked={task.done}
          onChange={(event) => onToggle(event.target.checked)}
          aria-label={task.title}
        />

        <button type="button" onClick={onOpen} className="min-w-0 flex-1 py-0.5 text-left">
          <span
            className={`block break-words text-sm leading-snug ${
              task.done ? 'text-secondary line-through' : 'text-primary'
            }`}
          >
            {task.title}
          </span>
          {task.notes ? <span className="mt-0.5 block truncate text-xs text-secondary">{task.notes}</span> : null}
          {task.due_at > 0 ? (
            <span
              className={`mt-1 inline-block rounded px-1.5 py-0.5 text-[0.6875rem] font-medium ${
                overdue ? 'bg-rose-500/10 text-rose-600 dark:text-rose-400' : 'bg-raised text-secondary'
              }`}
            >
              {formatDueDate(task.due_at)}
            </span>
          ) : null}
        </button>

        <div className="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
          {task.thread_id && onOpenMessage ? (
            <IconButton label={t('tasks.openMessage')} icon={Mail} size="sm" onClick={onOpenMessage} />
          ) : null}
          <IconButton label={t('tasks.deleteTask')} icon={Trash2} size="sm" variant="danger" onClick={onDelete} />
        </div>
      </div>

      {expanded ? children : null}
    </div>
  )
}
