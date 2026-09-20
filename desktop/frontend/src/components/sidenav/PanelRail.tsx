import { ListTodo } from 'lucide-react'
import { useValue } from '@legendapp/state/react'

import { useTranslation } from '../../lib/i18n'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import { toggleTasksPanel } from '../../states/tasks'

/**
 * The right-hand rail: what side panels there are, and which one is showing.
 *
 * Separate from the left rail because the two answer different questions. The
 * left one is "what am I looking at" — one account or board at a time, and
 * picking a new one replaces the last. This one is "what is open beside it",
 * which is a toggle, and which lives on the side the panel opens from.
 *
 * It renders nothing when no panel is available, so an app with Tasks switched
 * off has no empty strip down its edge.
 */
export function PanelRail() {
  const { t } = useTranslation()
  const tasksEnabled = useValue(settings$.tasksEnabled)
  const tasksPanelOpen = useValue(ui$.tasksPanelOpen)

  if (!tasksEnabled) return null

  return (
    <aside className="flex w-[52px] shrink-0 flex-col items-center gap-2 border-l border-border bg-sidenav py-3 max-[900px]:hidden select-none">
      <PanelRailButton
        label={t('tasks.title')}
        active={tasksPanelOpen}
        onClick={toggleTasksPanel}
        icon={<ListTodo size={19} />}
      />
    </aside>
  )
}

function PanelRailButton({
  label,
  active,
  onClick,
  icon,
}: {
  label: string
  active: boolean
  onClick: () => void
  icon: React.ReactNode
}) {
  return (
    <div className="group relative flex w-full justify-center">
      <div
        className={`absolute right-0 top-1/2 w-1 -translate-y-1/2 rounded-l bg-accent transition-all duration-200 ${
          active ? 'h-7' : 'h-0 group-hover:h-3'
        }`}
      />
      <button
        type="button"
        onClick={onClick}
        title={label}
        aria-label={label}
        aria-pressed={active}
        className={`flex h-10 w-10 cursor-pointer items-center justify-center rounded-2xl transition-all duration-200 ${
          active
            ? 'bg-accent text-white'
            : 'bg-white/10 text-white/60 hover:scale-105 hover:bg-white/20 hover:text-white'
        }`}
      >
        {icon}
      </button>
    </div>
  )
}
