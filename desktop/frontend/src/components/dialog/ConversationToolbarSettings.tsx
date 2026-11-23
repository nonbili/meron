import { useLayoutEffect, useRef } from 'react'
import { DndContext, closestCenter } from '@dnd-kit/core'
import { restrictToVerticalAxis } from '@dnd-kit/modifiers'
import { SortableContext, arrayMove, useSortable, verticalListSortingStrategy } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { GripVertical } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { CONVERSATION_TOOLBAR_ACTIONS, settings$ } from '../../states/settings'
import type { ConversationToolbarAction } from '../../states/settings'
import { CONVERSATION_ACTION_META } from '../chat/conversationActions'
import { useTaskDragSensors } from '../tasks/taskDrag'
import { SettingsGroup, Switch } from './AccountSettingsRows'

// Which thread actions the conversation toolbar shows, and in what order. The
// pinned ones come first, in their order, dragged by their handles; the rest
// follow in the app's own order, and a switch pins one to the end.
export function ConversationToolbarSettings() {
  const { t } = useTranslation()
  const pinned = useValue(settings$.conversationToolbar)
  const sensors = useTaskDragSensors()
  const unpinned = CONVERSATION_TOOLBAR_ACTIONS.filter((id) => !pinned.includes(id))
  const title = t('settings.appearance.conversationToolbar')

  const move = (from: number, to: number) => {
    if (from < 0 || to < 0 || from >= pinned.length || to >= pinned.length || from === to) return
    settings$.conversationToolbar.set(arrayMove(pinned, from, to))
  }
  const toggle = (id: ConversationToolbarAction) =>
    settings$.conversationToolbar.set(pinned.includes(id) ? pinned.filter((item) => item !== id) : [...pinned, id])

  return (
    <SettingsGroup title={title}>
      <p data-setting-title={title} className="px-3.5 py-2.5 text-[0.6875rem] text-secondary">
        {t('settings.appearance.conversationToolbarHint')}
      </p>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        modifiers={[restrictToVerticalAxis]}
        onDragEnd={({ active, over }) => {
          if (!over) return
          move(
            pinned.indexOf(active.id as ConversationToolbarAction),
            pinned.indexOf(over.id as ConversationToolbarAction),
          )
        }}
      >
        <SortableContext items={pinned} strategy={verticalListSortingStrategy}>
          {pinned.map((id, at) => (
            <ActionRow key={id} id={id} at={at} pinned onMove={(to) => move(at, to)} onToggle={() => toggle(id)} />
          ))}
        </SortableContext>
      </DndContext>
      {unpinned.map((id) => (
        <ActionRow key={id} id={id} at={-1} pinned={false} onToggle={() => toggle(id)} />
      ))}
    </SettingsGroup>
  )
}

// One action: its handle while pinned (the arrow keys move it too, as in the
// rules list, rather than dnd-kit's keyboard sensor, which would take Escape
// from the dialog), its icon and name, and the switch that pins it.
function ActionRow({
  id,
  at,
  pinned,
  onMove,
  onToggle,
}: {
  id: ConversationToolbarAction
  at: number
  pinned: boolean
  onMove?: (to: number) => void
  onToggle: () => void
}) {
  const { t } = useTranslation()
  const { listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({
    id,
    disabled: !pinned,
  })
  const handle = useRef<HTMLButtonElement | null>(null)
  const keepFocus = useRef(false)
  // Moving a row in the DOM can drop the focus it holds.
  useLayoutEffect(() => {
    if (keepFocus.current) handle.current?.focus()
    keepFocus.current = false
  }, [at])

  const meta = CONVERSATION_ACTION_META[id]
  const label = t(meta.labelKey)
  return (
    <div
      ref={pinned ? setNodeRef : undefined}
      style={
        pinned
          ? { transform: CSS.Translate.toString(transform), transition: isDragging ? 'none' : transition }
          : undefined
      }
      className={`flex min-h-11 items-center gap-2.5 px-3.5 py-2 ${isDragging ? 'relative z-10 bg-raised' : ''}`}
    >
      {pinned ? (
        <button
          ref={(node) => {
            handle.current = node
            setActivatorNodeRef(node)
          }}
          type="button"
          title={t('common.dragToReorder')}
          aria-label={t('common.dragToReorder')}
          aria-keyshortcuts="ArrowUp ArrowDown"
          className="-ml-1.5 flex h-6 w-4 shrink-0 cursor-grab touch-none items-center justify-center rounded text-secondary hover:text-primary active:cursor-grabbing"
          {...listeners}
          onKeyDown={(event) => {
            if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
            event.preventDefault()
            keepFocus.current = true
            onMove?.(at + (event.key === 'ArrowUp' ? -1 : 1))
          }}
        >
          <GripVertical size={13} />
        </button>
      ) : (
        // Lines the unpinned rows up with the pinned ones' handles.
        <span className="-ml-1.5 w-4 shrink-0" />
      )}
      <meta.icon size={15} className="shrink-0 text-secondary" />
      <span className="min-w-0 flex-1 truncate text-xs text-primary">{label}</span>
      <Switch checked={pinned} onChange={onToggle} label={label} />
    </div>
  )
}
