import { Fragment, type ReactNode } from 'react'
import { Check } from 'lucide-react'

import { MenuItem } from '../menu/MenuItem'
import { taskListGroups, type TaskList } from '../../states/tasks'

/** A menu's task lists: the ones kept on this device first, then each synced
 * account's under its address, so lists named alike (every Google account
 * has a "My Tasks") can be told apart. */
export function TaskListMenuItems({
  lists,
  selected,
  icon,
  onPick,
}: {
  lists: TaskList[]
  selected?: string
  icon?: ReactNode
  onPick: (id: string) => void
}) {
  return taskListGroups(lists).map((group) => (
    <Fragment key={group.key}>
      {group.label && (
        <div
          role="presentation"
          className="mt-1 truncate border-t border-border/60 px-2 pb-1 pt-2 text-[0.6875rem] font-semibold text-secondary"
          title={group.label}
        >
          {group.label}
        </div>
      )}
      {group.lists.map((list) => (
        <MenuItem
          key={list.id}
          icon={icon}
          label={list.title}
          trailing={list.id === selected ? <Check size={13} className="text-accent" /> : undefined}
          onClick={() => onPick(list.id)}
        />
      ))}
    </Fragment>
  ))
}
