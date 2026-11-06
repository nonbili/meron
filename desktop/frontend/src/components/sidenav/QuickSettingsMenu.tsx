import { Columns3, FolderOpen, Info, Plus, RefreshCw, Settings, SquareChevronRight } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { invoke } from '../../lib/bridge'
import { showToast } from '../../states/ui'
import { syncMail } from '../../states/mail'
import { openCommandPalette, ui$ } from '../../states/ui'
import { formatShortcut, isMac, type ShortcutId } from '../../lib/shortcuts'
import { MenuItem } from '../menu/MenuItem'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'

/** The chord a menu row triggers, so the keystroke is learnable from the menu. */
function Hint({ id }: { id: ShortcutId }) {
  return (
    <kbd className="shrink-0 text-[0.625rem] font-normal text-secondary/60">
      {formatShortcut(id).join(isMac ? '' : '+')}
    </kbd>
  )
}

/**
 * Popover with common app actions. Shared by the desktop side navigation 3-dot menu and
 * the narrow-window thread-list header. `placement` flips the panel above
 * ("up") or below ("down") the anchor.
 */
export function QuickSettingsMenu({
  anchor,
  onAddKanbanBoard,
  onClose,
}: {
  anchor: { x: number; y: number; placement: 'up' | 'down' }
  onAddKanbanBoard?: () => void
  onClose: () => void
}) {
  const { t } = useTranslation()
  const busy = useValue(ui$.busy)

  return (
    <FloatingContextMenu
      x={anchor.x}
      y={anchor.placement === 'up' ? anchor.y - 4 : anchor.y + 4}
      placement={anchor.placement}
      onClose={onClose}
      overlay
      className="fixed z-50 max-h-[calc(100dvh-16px)] w-60 overflow-y-auto rounded-lg p-2"
      onContextMenu={(e) => {
        e.preventDefault()
        e.stopPropagation()
      }}
    >
      <MenuItem
        icon={<SquareChevronRight size={13} className="text-secondary" />}
        label={t('palette.label')}
        trailing={<Hint id="palette.open" />}
        onClick={() => {
          onClose()
          openCommandPalette()
        }}
      />
      <MenuItem
        icon={<FolderOpen size={13} className="text-secondary" />}
        label={t('eml.open')}
        onClick={() => {
          onClose()
          void invoke('eml.open').catch((error) => showToast(String(error), 'error'))
        }}
      />
      <MenuItem
        icon={<Plus size={13} className="text-secondary" />}
        label={t('accounts.actions.addAccount')}
        onClick={() => {
          ui$.setupOpen.set(true)
          onClose()
        }}
      />
      {onAddKanbanBoard && (
        <MenuItem
          icon={<Columns3 size={13} className="text-secondary" />}
          label={t('kanban.actions.addBoard')}
          onClick={() => {
            onAddKanbanBoard()
            onClose()
          }}
        />
      )}
      <MenuItem
        icon={<RefreshCw size={13} className={busy ? 'animate-spin text-accent' : 'text-secondary'} />}
        label={busy ? t('threads.actions.syncing') : t('threads.actions.syncMailbox')}
        trailing={<Hint id="mail.sync" />}
        onClick={() => syncMail()}
      />
      <MenuItem
        icon={<Settings size={13} className="text-secondary" />}
        label={t('settings.label')}
        trailing={<Hint id="settings.open" />}
        onClick={() => {
          ui$.settingsOpen.set(true)
          onClose()
        }}
      />
      <MenuItem
        icon={<Info size={13} className="text-secondary" />}
        label={t('about.title')}
        onClick={() => {
          ui$.aboutOpen.set(true)
          onClose()
        }}
      />
    </FloatingContextMenu>
  )
}
