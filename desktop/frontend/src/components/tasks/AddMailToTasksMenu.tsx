import { useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { Check, ChevronRight, ListTodo } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { addTaskFromMessage, loadTaskLists, type TaskList } from '../../states/tasks'
import { ui$ } from '../../states/ui'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'

export function AddMailToTasksMenu({
  title,
  account,
  threadId,
  onAdded,
}: {
  title: string
  account: string
  threadId: string
  onAdded: () => void
}) {
  const { t } = useTranslation()
  const selectedList = useValue(ui$.activeTaskList)
  const [lists, setLists] = useState<TaskList[]>([])
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    let active = true
    void loadTaskLists()
      .then((result) => {
        if (active) setLists(result.lists)
      })
      .catch(() => {})
    return () => {
      active = false
    }
  }, [])

  function open() {
    const rect = ref.current?.getBoundingClientRect()
    if (lists.length < 2 || !rect) return
    setAnchor({ x: rect.right + 204 > window.innerWidth ? rect.left - 200 : rect.right, y: rect.top })
  }
  function pick(listId: string) {
    onAdded()
    void addTaskFromMessage({ title, account, threadId, listId })
  }

  return (
    <div ref={ref} onMouseEnter={open} onMouseLeave={() => setAnchor(null)}>
      <MenuItem
        icon={<ListTodo size={13} className="text-secondary" />}
        label={t('tasks.addFromMessage')}
        disabled={!lists.length}
        trailing={lists.length > 1 ? <ChevronRight size={13} /> : undefined}
        onClick={() => (lists.length === 1 ? pick(lists[0].id) : open())}
      />
      {anchor && (
        <FloatingContextMenu
          x={anchor.x}
          y={anchor.y}
          dataAttribute="data-thread-context-menu"
          className="fixed z-[51] max-h-[calc(100vh-1rem)] w-[200px] overflow-y-auto rounded-xl border border-border bg-chats p-1 shadow-xl"
        >
          {lists.map((list) => (
            <MenuItem
              key={list.id}
              label={list.title}
              trailing={list.id === selectedList ? <Check size={13} /> : undefined}
              onClick={() => pick(list.id)}
            />
          ))}
        </FloatingContextMenu>
      )}
    </div>
  )
}
