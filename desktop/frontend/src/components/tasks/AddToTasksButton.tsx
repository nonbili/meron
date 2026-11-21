import { useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { Check, ListPlus } from 'lucide-react'
import { errorMessage } from '../../lib/errors'
import { useTranslation } from '../../lib/i18n'
import {
  addTaskFromMessage,
  loadTaskLists,
  taskListLabel,
  warnIfThreadInTasks,
  type TaskList,
} from '../../states/tasks'
import { showToast, ui$ } from '../../states/ui'
import { IconButton } from '../button/IconButton'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'

// The conversation header's twin of AddMailToTasksMenu. Lists are read on press
// rather than on mount, so one made in the panel meanwhile is offered; with a
// single list the press adds straight away, otherwise it asks which. A thread
// with an open task already is only pointed out, not added again.
export function AddToTasksButton({ title, account, threadId }: { title: string; account: string; threadId: string }) {
  const { t } = useTranslation()
  const selectedList = useValue(ui$.activeTaskList)
  const [menu, setMenu] = useState<{ x: number; y: number; lists: TaskList[] } | null>(null)
  // A press spans two awaits before anything is filed: a second press meanwhile
  // is dropped, and one that outlives its thread being on screen files nothing.
  const busy = useRef(false)
  const shownThread = useRef(threadId)
  shownThread.current = threadId
  useEffect(() => setMenu(null), [threadId])

  async function add(listId: string) {
    setMenu(null)
    try {
      await addTaskFromMessage({ title, account, threadId, listId, checked: true })
    } catch (error) {
      showToast(errorMessage(error, t('tasks.addFromMessage')), 'error')
    }
  }

  async function press(button: HTMLButtonElement) {
    if (busy.current) return
    busy.current = true
    try {
      // Before the list picker, so a thread already filed isn't sent through it.
      if (await warnIfThreadInTasks(threadId)) return
      const result = await loadTaskLists().catch(() => ({ lists: [] as TaskList[] }))
      const lists = result.lists.filter((list) => !list.read_only)
      if (shownThread.current !== threadId || !lists.length) return
      if (lists.length === 1) return await add(lists[0].id)
      const rect = button.getBoundingClientRect()
      setMenu({ x: rect.right - 200, y: rect.bottom + 8, lists })
    } finally {
      busy.current = false
    }
  }

  return (
    <>
      <IconButton
        icon={ListPlus}
        label={t('tasks.addFromMessage')}
        active={!!menu}
        onClick={(event) => void press(event.currentTarget)}
      />
      {menu && (
        <FloatingContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          overlay
          overlayClassName="fixed inset-0 z-[60]"
          className="fixed z-[61] max-h-[calc(100vh-1rem)] w-[200px] overflow-y-auto shadow-xl"
        >
          {menu.lists.map((list) => (
            <MenuItem
              key={list.id}
              label={taskListLabel(list)}
              trailing={list.id === selectedList ? <Check size={13} /> : undefined}
              onClick={() => void add(list.id)}
            />
          ))}
        </FloatingContextMenu>
      )}
    </>
  )
}
