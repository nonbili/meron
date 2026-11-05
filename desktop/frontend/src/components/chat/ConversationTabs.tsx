import { useEffect, useRef, useState } from 'react'
import { DndContext, closestCenter } from '@dnd-kit/core'
import { restrictToHorizontalAxis } from '@dnd-kit/modifiers'
import { SortableContext, horizontalListSortingStrategy, useSortable } from '@dnd-kit/sortable'
import { CSS as DndCSS } from '@dnd-kit/utilities'
import { MessageSquare, SquarePen, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import {
  closeMessageTab,
  closeMessageTabsKeepingDrafts,
  activateConversationTab,
  moveMessageTab,
} from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { closeCurrentConversation, kanban$ } from '../../states/kanban'
import { ui$ } from '../../states/ui'
import type { MessageTab } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import { useTaskDragSensors } from '../tasks/taskDrag'

// A wheel line or page in pixels, for devices that report deltas in those
// units (deltaMode 1 and 2) rather than pixels.
const WHEEL_LINE_PX = 40

// The underline carries the accent; the label stays in the text colour so the
// active tab doesn't compete with the subject heading below it.
const ACTIVE_TAB = 'border-accent font-semibold text-primary'
const INACTIVE_TAB = 'border-transparent font-medium text-secondary hover:text-primary'
// Like browser tabs: all the same width, sharing the strip as more open (the
// strip scrolls past min-w-24), so a tab keeps its width when it becomes active.
// In rem so a larger text size gets the same room.
const TAB_WIDTH = 'w-[12.5rem] min-w-24'
// A long label fades out at the tab's end instead of an ellipsis. The label
// fills the tab, so a short one never reaches the fade.
const LABEL_FADE = '[mask-image:linear-gradient(to_right,black_calc(100%-1rem),transparent)]'
// Where the × shows, the fade ends 1rem early, clear of the × and its shadow.
const LABEL_FADE_BEFORE_X = '[mask-image:linear-gradient(to_right,black_calc(100%-2rem),transparent_calc(100%-1rem))]'
const LABEL_FADE_ON_REVEAL =
  'group-hover:[mask-image:linear-gradient(to_right,black_calc(100%-2rem),transparent_calc(100%-1rem))] group-focus-within:[mask-image:linear-gradient(to_right,black_calc(100%-2rem),transparent_calc(100%-1rem))]'

function labelClass(active: boolean) {
  const fade = active ? LABEL_FADE_BEFORE_X : `${LABEL_FADE} ${LABEL_FADE_ON_REVEAL}`
  return `min-w-0 flex-1 overflow-hidden whitespace-nowrap text-left ${fade}`
}

// Scrolls the strip, and only the strip, just enough to show the tab.
function revealTab(strip: HTMLElement, tabId: string) {
  const tab = strip.querySelector<HTMLElement>(`[data-tab-id="${CSS.escape(tabId)}"]`)
  if (!tab) return
  const stripRect = strip.getBoundingClientRect()
  const tabRect = tab.getBoundingClientRect()
  if (tabRect.left < stripRect.left) strip.scrollLeft -= stripRect.left - tabRect.left
  else if (tabRect.right > stripRect.right) strip.scrollLeft += tabRect.right - stripRect.right
}

// The tab strip above the conversation: the "Current" tab (when a conversation
// is open behind the tabs) plus open thread, reader and compose tabs. Renders
// nothing when no tabs are open.
export function ConversationTabs() {
  const { t } = useTranslation()
  const tabs = useValue(compose$.tabs)
  const activeTab = useValue(compose$.activeTab)
  const conversationThread = useValue(compose$.conversationThread)
  const activeBoardId = useValue(kanban$.activeBoardId)
  const paneThreadId = useValue(kanban$.paneThreadId)
  // The Current tab returns to the conversation the pane was showing before a
  // tab took over it. With no such conversation there is nothing to return to:
  // it would blank the pane, and in kanban view — where the pane only exists
  // while a card or a tab is open — close it outright. So it is only offered
  // once there is a conversation behind the tabs. Kanban owns its open
  // conversation through paneThreadId; elsewhere it's the remembered thread.
  const hasCurrentConversation = activeBoardId ? !!paneThreadId : !!conversationThread
  const stripRef = useRef<HTMLDivElement>(null)

  // Keep the active tab visible, e.g. a newly opened one past the strip's
  // right edge. Size changes re-run this below.
  useEffect(() => {
    if (stripRef.current) revealTab(stripRef.current, activeTab)
  }, [activeTab, tabs.length])

  // The strip hides its scrollbar (it would sit on the active tab's
  // underline), so a vertical wheel scrolls it sideways instead, and a fade on
  // each edge with tabs past it hints there is more. The wheel listener is
  // native: React's onWheel is passive and can't stop the page scrolling.
  const hasTabs = tabs.length > 0
  const [overflow, setOverflow] = useState({ left: false, right: false })
  useEffect(() => {
    const strip = stripRef.current
    if (!strip) return
    const updateOverflow = () => {
      const left = strip.scrollLeft > 1
      const right = strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 1
      setOverflow((prev) => (prev.left === left && prev.right === right ? prev : { left, right }))
    }
    const onWheel = (event: WheelEvent) => {
      if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return
      if (strip.scrollWidth <= strip.clientWidth) return
      event.preventDefault()
      const unit =
        event.deltaMode === WheelEvent.DOM_DELTA_PAGE
          ? strip.clientWidth
          : event.deltaMode === WheelEvent.DOM_DELTA_LINE
            ? WHEEL_LINE_PX
            : 1
      strip.scrollLeft += event.deltaY * unit
    }
    updateOverflow()
    strip.addEventListener('wheel', onWheel, { passive: false })
    strip.addEventListener('scroll', updateOverflow, { passive: true })
    // Watch the tabs too: opening or closing one, or a compose tab's subject
    // growing, changes the scroll width without resizing the strip. Either
    // kind of change can push the active tab out of view.
    const onResize = () => {
      revealTab(strip, compose$.activeTab.peek())
      updateOverflow()
    }
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(onResize)
    observer?.observe(strip)
    for (const child of strip.children) observer?.observe(child)
    return () => {
      strip.removeEventListener('wheel', onWheel)
      strip.removeEventListener('scroll', updateOverflow)
      observer?.disconnect()
    }
  }, [hasTabs, tabs.length, hasCurrentConversation])

  const [menu, setMenu] = useState<{ x: number; y: number; tabId: string } | null>(null)
  const sensors = useTaskDragSensors()

  if (!hasTabs) return null

  return (
    <div className="relative shrink-0 border-b border-border/60 bg-header">
      {overflow.left && (
        <div className="pointer-events-none absolute inset-y-0 left-0 z-10 w-8 bg-linear-to-r from-header to-transparent" />
      )}
      {overflow.right && (
        <div className="pointer-events-none absolute inset-y-0 right-0 z-10 w-8 bg-linear-to-l from-header to-transparent" />
      )}
      <div ref={stripRef} className="flex h-9 items-stretch gap-1 no-scrollbar overflow-x-auto px-2 select-none">
        {hasCurrentConversation && (
          <div
            data-tab-id=""
            onClick={() => activateConversationTab()}
            onContextMenu={(event) => {
              event.preventDefault()
              setMenu({ x: event.clientX, y: event.clientY, tabId: '' })
            }}
            className={`group relative flex ${TAB_WIDTH} items-center gap-1.5 px-3 text-xs border-b-2 transition-colors cursor-pointer ${
              activeTab === '' ? ACTIVE_TAB : INACTIVE_TAB
            }`}
            title={t('chat.currentConversation')}
          >
            <button className="flex min-w-0 flex-1 items-center gap-1.5 self-stretch cursor-pointer">
              <MessageSquare size={13} className="shrink-0" />
              <span className={labelClass(activeTab === '')}>{t('chat.current')}</span>
            </button>
            <button
              onClick={(event) => {
                event.stopPropagation()
                closeCurrentConversation()
              }}
              className={`absolute right-1 top-1/2 z-10 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded-full bg-header text-secondary shadow-[-4px_0_4px_var(--color-header)] hover:bg-active hover:text-primary ${
                activeTab === ''
                  ? ''
                  : 'pointer-events-none opacity-0 group-hover:pointer-events-auto group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:opacity-100'
              }`}
              title={t('chat.closeTab')}
              aria-label={t('chat.closeTab')}
            >
              <X size={11} />
            </button>
          </div>
        )}
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          modifiers={[restrictToHorizontalAxis]}
          onDragEnd={({ active, over }) => {
            if (over) moveMessageTab(String(active.id), String(over.id))
          }}
        >
          <SortableContext items={tabs.map((tab) => tab.id)} strategy={horizontalListSortingStrategy}>
            {tabs.map((tab) => (
              <SortableTab
                key={tab.id}
                tab={tab}
                active={activeTab === tab.id}
                onMenu={(x, y) => setMenu({ x, y, tabId: tab.id })}
              />
            ))}
          </SortableContext>
        </DndContext>
      </div>
      {menu && (
        <TabContextMenu {...menu} tabs={tabs} hasCurrent={hasCurrentConversation} onClose={() => setMenu(null)} />
      )}
    </div>
  )
}

// One open tab, dragged anywhere but its × to reorder. Current isn't one of
// these: it stays first, ahead of whatever is reordered.
function SortableTab({
  tab,
  active,
  onMenu,
}: {
  tab: MessageTab
  active: boolean
  onMenu: (x: number, y: number) => void
}) {
  const { t } = useTranslation()
  const { listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: tab.id })

  return (
    <div
      ref={setNodeRef}
      data-tab-id={tab.id}
      style={{ transform: DndCSS.Translate.toString(transform), transition: isDragging ? 'none' : transition }}
      onClick={() => {
        // Activate the tab before retargeting selectedThread so the Current
        // tab's remembered thread (conversationThread) isn't overwritten.
        compose$.activeTab.set(tab.id)
        if (tab.kind === 'thread') ui$.selectedThread.set(tab.threadId)
      }}
      className={`group relative flex ${TAB_WIDTH} cursor-pointer items-center gap-1.5 px-3 text-xs border-b-2 transition-colors ${
        active ? ACTIVE_TAB : INACTIVE_TAB
      } ${isDragging ? 'z-20 bg-header' : ''}`}
      onContextMenu={(event) => {
        event.preventDefault()
        onMenu(event.clientX, event.clientY)
      }}
      title={tab.subject}
      {...listeners}
    >
      {tab.kind === 'compose' ? (
        <SquarePen size={12} className="shrink-0" />
      ) : (
        <Avatar
          name={tab.from}
          email={tab.fromAddr}
          src={tab.feedIcon ? `/media/${tab.feedIcon}` : undefined}
          size={18}
        />
      )}
      <span className={labelClass(active)}>{tab.subject}</span>
      <button
        data-no-drag
        onClick={(event) => {
          event.stopPropagation()
          void closeMessageTab(tab.id)
        }}
        // Overlay the label so revealing the × never consumes tab width.
        className={`absolute right-1 top-1/2 z-10 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded-full bg-header text-secondary shadow-[-4px_0_4px_var(--color-header)] hover:bg-active hover:text-primary ${
          active
            ? ''
            : 'pointer-events-none opacity-0 group-hover:pointer-events-auto group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:opacity-100'
        }`}
        title={t('chat.closeTab')}
      >
        <X size={11} />
      </button>
    </div>
  )
}

// Right-click menu for a tab, the Current tab ('') included: it sits first in
// the strip and closes like the conversation header's X. The bulk entries
// close compose tabs too, keeping their messages in Drafts (see
// closeMessageTabsKeepingDrafts), and are disabled when there is nothing to
// close.
function TabContextMenu({
  x,
  y,
  tabId,
  tabs,
  hasCurrent,
  onClose,
}: {
  x: number
  y: number
  tabId: string
  tabs: MessageTab[]
  hasCurrent: boolean
  onClose: () => void
}) {
  const { t } = useTranslation()
  const entries = [...(hasCurrent ? [''] : []), ...tabs.map((tab) => tab.id)]
  const index = entries.indexOf(tabId)
  if (index === -1) return null
  const close = (ids: string[]) => {
    onClose()
    // Tabs first: closing Current hands the pane to a tab still open.
    closeMessageTabsKeepingDrafts(ids.filter((id) => id !== ''))
    if (ids.includes('')) closeCurrentConversation()
  }
  const bulk = [
    { label: t('chat.closeOtherTabs'), ids: entries.filter((id) => id !== tabId) },
    { label: t('chat.closeTabsToLeft'), ids: entries.slice(0, index) },
    { label: t('chat.closeTabsToRight'), ids: entries.slice(index + 1) },
    { label: t('chat.closeAllTabs'), ids: entries },
  ]

  return (
    <FloatingContextMenu
      x={x}
      y={y}
      offset={4}
      onClose={onClose}
      overlay
      className="fixed z-50 min-w-[176px]"
      onContextMenu={(event) => {
        event.preventDefault()
        event.stopPropagation()
      }}
    >
      <MenuItem
        label={t('chat.closeTab')}
        onClick={() => {
          if (tabId !== '') {
            onClose()
            void closeMessageTab(tabId)
          } else close([''])
        }}
      />
      {bulk.map((item) => (
        <MenuItem
          key={item.label}
          label={item.label}
          className="disabled:cursor-not-allowed disabled:opacity-50"
          disabled={item.ids.length === 0}
          onClick={() => close(item.ids)}
        />
      ))}
    </FloatingContextMenu>
  )
}
