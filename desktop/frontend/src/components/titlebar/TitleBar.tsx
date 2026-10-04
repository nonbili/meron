import { useState, type Dispatch, type SetStateAction } from 'react'
import { createPortal } from 'react-dom'
import { Menu, SquareCheckBig } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import logo from '../../assets/logo.png'
import { useTranslation } from '../../lib/i18n'
import { isMac } from '../../lib/shortcuts'
import { windowChrome$ } from '../../lib/windowChrome'
import { kanban$ } from '../../states/kanban'
import { settings$ } from '../../states/settings'
import { toggleTasksPanel } from '../../states/tasks'
import { ui$ } from '../../states/ui'
import { KanbanSearch } from '../kanban/KanbanSearch'
import { BoardDialog, type BoardDialogState } from '../sidenav/BoardDialog'
import { QuickSettingsMenu } from '../sidenav/QuickSettingsMenu'
import { ThreadSearchInput } from '../threads/ThreadSearchInput'
import { WindowControls } from './WindowControls'

/**
 * Whether Meron draws its own title bar: always on macOS (the native one is
 * hidden, see main.go), and on Linux and Windows while the integrated title
 * bar is in effect. Elsewhere the system title bar stays and the side navigation keeps
 * the buttons the title bar would hold.
 */
export function useTitleBar(): boolean {
  const integrated = useValue(windowChrome$.integrated)
  return isMac || integrated
}

type MenuPosition = { x: number; y: number } | null

/**
 * The window's top row, in the side navigation's colors so the two read as
 * one frame. It moves the window ([data-titlebar] in index.css) and holds the
 * search box of the thread list or kanban board, the app-wide Tasks and quick-settings buttons, plus
 * the window controls on Linux and Windows.
 * On macOS the native traffic lights float over its left end; the row leaves
 * a little extra room for the search field and app buttons.
 */
export function TitleBar({ tools = true }: { tools?: boolean }) {
  const { t } = useTranslation()
  const [menu, setMenu] = useState<MenuPosition>(null)
  const [boardDialog, setBoardDialog] = useState<BoardDialogState | null>(null)
  const shown = useTitleBar()
  const windows = useValue(windowChrome$.platform) === 'windows'
  const kanbanBoard = useValue(kanban$.activeBoardId)
  const startControls = useValue(windowChrome$.layout.start)
  if (!shown) return null
  // A 40px bar on every platform leaves 4px above and below the 32px search,
  // which is at most 28rem wide.
  // Windows caption buttons stay flush with the right edge.
  const layout = isMac ? 'px-2' : windows ? 'pl-1.5' : 'px-1.5'
  return (
    <div
      data-titlebar
      // Three columns, the outer two equal while there is room, so the search
      // is centred on the window rather than between the ends' unequal contents.
      // Each end keeps at least its own width; the search gives way first.
      className={`grid h-10 shrink-0 grid-cols-[minmax(max-content,1fr)_minmax(0,28rem)_minmax(max-content,1fr)] items-center bg-sidenav text-sidenav-ink ${layout}`}
      onContextMenu={(event) => {
        // The search box keeps the webview's own menu for its text.
        if (!tools || event.defaultPrevented || event.target instanceof HTMLInputElement) return
        event.preventDefault()
        setMenu({ x: event.clientX, y: event.clientY })
      }}
    >
      <div className="flex h-full items-center">
        {/* Clear of the traffic lights, which float over the left end. */}
        {isMac && <div className="w-18 shrink-0" />}
        <WindowControls side="start" />
        {/* Weight for the left end, across from the buttons on the right, when no
          window controls sit there. macOS has its traffic lights. Centred over
          the 60px side navigation's tiles: 6px row padding + margin + half the logo. */}
        {!isMac && startControls.length === 0 && (
          <img
            src={logo}
            alt=""
            draggable={false}
            className={`shrink-0 ${windows ? 'ml-4 h-4 w-4' : 'ml-3.5 h-5 w-5'}`}
          />
        )}
      </div>
      {/* The search of whatever is open: the thread list, or a kanban board.
        32px on every platform. */}
      <div className="flex h-8 min-w-0 px-2">
        {tools && (kanbanBoard ? <KanbanSearch boardId={kanbanBoard} compact /> : <ThreadSearchInput compact />)}
      </div>
      <div className="flex h-full items-center justify-end">
        {tools && <TitleBarTools menu={menu} setMenu={setMenu} />}
        <WindowControls side="end" />
      </div>
      {menu && (
        <QuickSettingsMenu
          anchor={{ x: menu.x, y: menu.y, placement: 'down' }}
          onAddKanbanBoard={() => setBoardDialog({ mode: 'create', name: t('kanban.board.defaultName') })}
          onClose={() => setMenu(null)}
        />
      )}
      {/* Out of the title bar, which would make the dialog move the window. */}
      {boardDialog &&
        createPortal(
          <BoardDialog state={boardDialog} onChange={setBoardDialog} onClose={() => setBoardDialog(null)} />,
          document.body,
        )}
    </div>
  )
}

function TitleBarTools({ menu, setMenu }: { menu: MenuPosition; setMenu: Dispatch<SetStateAction<MenuPosition>> }) {
  const { t } = useTranslation()
  const windows = useValue(windowChrome$.platform) === 'windows'
  const tasksEnabled = useValue(settings$.tasksEnabled)
  const tasksPanelOpen = useValue(ui$.tasksPanelOpen)
  // As tall as the search, so hover highlights line up with it.
  const button = 'flex h-8 w-8 shrink-0 items-center justify-center rounded-lg transition-colors cursor-pointer'
  const idle = 'text-sidenav-ink/60 hover:bg-sidenav-ink/10 hover:text-sidenav-ink'
  const pressed = 'bg-sidenav-ink/15 text-sidenav-ink'
  const iconSize = 17

  return (
    <div className={`flex items-center gap-1 ${isMac ? '' : windows ? 'mr-2' : 'mr-1.5'}`}>
      {tasksEnabled && (
        // Hidden with the panel itself, which doesn't fit below 900px.
        <button
          type="button"
          className={`${button} ${tasksPanelOpen ? pressed : idle} max-[900px]:hidden`}
          onClick={toggleTasksPanel}
          title={t('tasks.title')}
          aria-label={t('tasks.title')}
          aria-pressed={tasksPanelOpen}
        >
          <SquareCheckBig size={iconSize} />
        </button>
      )}
      <button
        type="button"
        className={`${button} ${menu ? pressed : idle}`}
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect()
          // QuickSettingsMenu is w-60; line its right edge up with the button.
          setMenu({ x: rect.right - 240, y: rect.bottom })
        }}
        title={t('common.more')}
        aria-label={t('common.more')}
      >
        <Menu size={iconSize} />
      </button>
    </div>
  )
}
