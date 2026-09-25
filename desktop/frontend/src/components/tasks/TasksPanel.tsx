import { useEffect, useMemo, useRef, useState } from 'react'
import { DndContext, PointerSensor, closestCenter, useSensor, useSensors } from '@dnd-kit/core'
import { restrictToVerticalAxis } from '@dnd-kit/modifiers'
import { SortableContext, verticalListSortingStrategy } from '@dnd-kit/sortable'
import { useValue } from '@legendapp/state/react'
import { Check, ChevronDown, ChevronRight, ListTodo, MoreHorizontal, Pencil, Plus, Trash2, X } from 'lucide-react'

import { useTranslation } from '../../lib/i18n'
import { IconButton } from '../button/IconButton'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import { ui$ } from '../../states/ui'
import {
  addTask,
  clearCompletedTasks,
  closeTasksPanel,
  createTaskList,
  deleteTask,
  deleteTaskList,
  loadTasks,
  openTaskMail,
  renameTaskList,
  reorderTasks,
  setTaskDone,
  tasks$,
} from '../../states/tasks'
import { TaskEditor } from './TaskEditor'
import { TaskRow } from './TaskRow'

/**
 * Fixed, like the panel it is modelled on. The thread list and conversation
 * already own the resizable widths in this window; a third draggable edge buys
 * little and costs the layout a lot.
 */
export const TASKS_PANEL_WIDTH = 340

type MenuAnchor = { x: number; y: number }

/**
 * Tasks as a right-hand panel, beside the mail or kanban view rather than in
 * place of it. That is the whole point of the shape: a list is worked against
 * the thread list it came from, and a task naming a message opens that message
 * in the conversation pane already on screen.
 */
export function TasksPanel({ listId }: { listId: string }) {
  const { t } = useTranslation()
  const lists = useValue(tasks$.lists)
  const items = useValue(tasks$.items)
  const editingId = useValue(tasks$.editingId)
  const [draft, setDraft] = useState('')
  const [renaming, setRenaming] = useState(false)
  const [listName, setListName] = useState('')
  const [listMenu, setListMenu] = useState<MenuAnchor | null>(null)
  const [actionsMenu, setActionsMenu] = useState<MenuAnchor | null>(null)
  // Collapsed by default, as in Google Tasks: done work is there to be found,
  // not to crowd the open tasks.
  const [completedOpen, setCompletedOpen] = useState(false)
  const listButtonRef = useRef<HTMLButtonElement | null>(null)
  const actionsButtonRef = useRef<HTMLButtonElement | null>(null)
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }))

  const activeList = lists.find((list) => list.id === listId)
  const open = useMemo(() => items.filter((task) => !task.done), [items])
  const completed = useMemo(() => items.filter((task) => task.done), [items])

  useEffect(() => {
    void loadTasks(listId)
    setRenaming(false)
  }, [listId])

  function submitDraft() {
    const title = draft.trim()
    if (!title) return
    setDraft('')
    void addTask(title)
  }

  // Anchor a popover under the control that opened it, rather than at the
  // pointer: these are buttons, not a right-click.
  function anchorUnder(element: HTMLElement | null): MenuAnchor {
    const box = element?.getBoundingClientRect()
    return { x: box?.left ?? 0, y: (box?.bottom ?? 0) + 4 }
  }

  return (
    <div
      // Hidden on a narrow window: at that width the thread list and
      // conversation already compete, and a third column would win nothing.
      className="flex min-h-0 shrink-0 flex-col border-l border-border bg-chats max-[900px]:hidden"
      style={{ width: TASKS_PANEL_WIDTH }}
    >
      <header className="flex h-12 shrink-0 items-center gap-1 border-b border-border pl-3 pr-1.5">
        {renaming ? (
          <RenameField
            value={listName}
            placeholder={t('tasks.listNamePlaceholder')}
            onChange={setListName}
            onCancel={() => setRenaming(false)}
            onCommit={() => {
              void renameTaskList(listId, listName)
              setRenaming(false)
            }}
            commitLabel={t('tasks.renameList')}
          />
        ) : (
          <>
            <button
              ref={listButtonRef}
              type="button"
              onClick={() => setListMenu(anchorUnder(listButtonRef.current))}
              className="group flex min-w-0 flex-1 items-center gap-1 rounded-lg px-1.5 py-1 text-left hover:bg-hover"
              title={activeList?.title ?? t('tasks.title')}
            >
              <span className="truncate text-sm font-semibold text-primary">
                {activeList?.title ?? t('tasks.title')}
              </span>
              <ChevronDown size={14} className="shrink-0 text-secondary" />
            </button>
            <IconButton
              ref={actionsButtonRef}
              label={t('chat.moreActions')}
              icon={MoreHorizontal}
              size="sm"
              onClick={() => setActionsMenu(anchorUnder(actionsButtonRef.current))}
            />
            <IconButton label={t('buttons.close')} icon={X} size="sm" onClick={closeTasksPanel} />
          </>
        )}
      </header>

      <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2">
        <Plus size={15} className="shrink-0 text-secondary" />
        <input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') submitDraft()
            if (event.key === 'Escape') setDraft('')
          }}
          placeholder={t('tasks.addTask')}
          className="min-w-0 flex-1 bg-transparent text-sm text-primary outline-none placeholder:text-secondary"
        />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto py-1">
        {items.length === 0 ? (
          <PanelEmptyState title={t('tasks.empty')} text={t('tasks.emptyHint')} />
        ) : (
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            modifiers={[restrictToVerticalAxis]}
            onDragEnd={({ active, over }) => {
              if (!over || active.id === over.id) return
              const ids = open.map((task) => task.id)
              const from = ids.indexOf(String(active.id))
              const to = ids.indexOf(String(over.id))
              if (from < 0 || to < 0) return
              const next = [...ids]
              next.splice(to, 0, ...next.splice(from, 1))
              void reorderTasks(next)
            }}
          >
            <SortableContext items={open.map((task) => task.id)} strategy={verticalListSortingStrategy}>
              {open.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  sortable
                  expanded={editingId === task.id}
                  onToggle={(done) => void setTaskDone(task.id, done)}
                  onOpen={() => tasks$.editingId.set(editingId === task.id ? '' : task.id)}
                  onDelete={() => void deleteTask(task.id)}
                  onOpenMessage={task.thread_id ? () => void openTaskMail(task.thread_id) : undefined}
                >
                  <TaskEditor task={task} lists={lists} />
                </TaskRow>
              ))}
            </SortableContext>
          </DndContext>
        )}
      </div>

      {/* Pinned under the scrolling list rather than at its end, so a long
          list can't push it out of sight. Opened, the completed tasks scroll
          on their own beneath it, up to half the panel. */}
      {completed.length > 0 ? (
        <div className="flex max-h-[50%] shrink-0 flex-col border-t border-border">
          <button
            type="button"
            onClick={() => setCompletedOpen((value) => !value)}
            className="flex w-full shrink-0 items-center gap-1 px-3 py-2 text-xs font-semibold text-secondary hover:text-primary"
          >
            {completedOpen ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
            {t('tasks.completed')}
            <span className="text-secondary">({completed.length})</span>
          </button>
          {completedOpen ? (
            <div className="min-h-0 overflow-y-auto pb-1">
              {completed.map((task) => (
                <TaskRow
                  key={task.id}
                  task={task}
                  sortable={false}
                  expanded={editingId === task.id}
                  onToggle={(done) => void setTaskDone(task.id, done)}
                  onOpen={() => tasks$.editingId.set(editingId === task.id ? '' : task.id)}
                  onDelete={() => void deleteTask(task.id)}
                  onOpenMessage={task.thread_id ? () => void openTaskMail(task.thread_id) : undefined}
                >
                  <TaskEditor task={task} lists={lists} />
                </TaskRow>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}

      {listMenu ? (
        <FloatingContextMenu
          x={listMenu.x}
          y={listMenu.y}
          onClose={() => setListMenu(null)}
          overlay
          className="fixed z-50 min-w-[200px] rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
        >
          {lists.map((list) => (
            <MenuItem
              key={list.id}
              icon={<ListTodo size={13} className="text-secondary" />}
              label={list.title}
              trailing={list.id === listId ? <Check size={13} className="text-accent" /> : undefined}
              onClick={() => {
                setListMenu(null)
                tasks$.editingId.set('')
                ui$.activeTaskList.set(list.id)
              }}
            />
          ))}
          <div className="my-1 border-t border-border" />
          <MenuItem
            icon={<Plus size={13} className="text-secondary" />}
            label={t('tasks.newList')}
            onClick={() => {
              setListMenu(null)
              void createTaskList(t('tasks.newList'))
            }}
          />
        </FloatingContextMenu>
      ) : null}

      {actionsMenu ? (
        <FloatingContextMenu
          x={actionsMenu.x}
          y={actionsMenu.y}
          onClose={() => setActionsMenu(null)}
          overlay
          className="fixed z-50 min-w-[200px] rounded-xl border border-border bg-chats p-1 shadow-xl animate-fade-in"
        >
          <MenuItem
            icon={<Pencil size={13} className="text-secondary" />}
            label={t('tasks.renameList')}
            onClick={() => {
              setActionsMenu(null)
              setListName(activeList?.title ?? '')
              setRenaming(true)
            }}
          />
          {completed.length > 0 ? (
            <MenuItem
              icon={<Trash2 size={13} className="text-secondary" />}
              label={t('tasks.clearCompleted')}
              onClick={() => {
                setActionsMenu(null)
                void clearCompletedTasks()
              }}
            />
          ) : null}
          <div className="my-1 border-t border-border" />
          <MenuItem
            danger
            icon={<Trash2 size={13} />}
            label={t('tasks.deleteList')}
            onClick={() => {
              setActionsMenu(null)
              void deleteTaskList(listId)
            }}
          />
        </FloatingContextMenu>
      ) : null}
    </div>
  )
}

function RenameField({
  value,
  placeholder,
  commitLabel,
  onChange,
  onCommit,
  onCancel,
}: {
  value: string
  placeholder: string
  commitLabel: string
  onChange: (value: string) => void
  onCommit: () => void
  onCancel: () => void
}) {
  return (
    <>
      <input
        autoFocus
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Escape') onCancel()
          if (event.key === 'Enter') onCommit()
        }}
        onBlur={onCancel}
        placeholder={placeholder}
        className="min-w-0 flex-1 rounded-lg bg-raised px-2 py-1 text-sm font-semibold text-primary outline-none focus:ring-1 focus:ring-accent"
      />
      {/* Mousedown, not click: the field's blur would otherwise cancel the
        rename before the click landed. */}
      <IconButton
        label={commitLabel}
        icon={Check}
        size="sm"
        onMouseDown={(event) => {
          event.preventDefault()
          onCommit()
        }}
      />
    </>
  )
}

/**
 * The shared `EmptyState` is sized for a full pane — a 64px icon tile and 32px
 * of padding — which crowds a 340px panel. This is the same idea at panel
 * scale.
 */
function PanelEmptyState({ title, text }: { title: string; text: string }) {
  return (
    <div className="flex flex-col items-center px-6 py-10 text-center select-none animate-fade-in">
      <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-raised text-accent">
        <ListTodo size={18} strokeWidth={1.8} />
      </div>
      <p className="mt-3 text-sm font-semibold text-primary">{title}</p>
      <p className="mt-1 text-xs leading-relaxed text-secondary">{text}</p>
    </div>
  )
}
