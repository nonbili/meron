import { Columns3, Info, Plus, RefreshCw, Settings } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from 'react-i18next'
import { syncMail } from '../../states/mail'
import { ui$ } from '../../states/ui'
import { MenuItem } from '../menu/MenuItem'

/**
 * Popover with common app actions. Shared by the desktop sidebar 3-dot menu and
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
    <>
      <div
        className="fixed inset-0 z-40"
        onClick={onClose}
        onContextMenu={(e) => {
          e.preventDefault()
          onClose()
        }}
      />
      <div
        className={`fixed z-50 w-52 rounded-lg border border-border bg-chats p-2 shadow-2xl animate-fade-in text-primary ${
          anchor.placement === 'up' ? '-translate-y-full' : ''
        }`}
        style={{ left: anchor.x, top: anchor.placement === 'up' ? anchor.y - 4 : anchor.y + 4 }}
        onContextMenu={(e) => {
          e.preventDefault()
          e.stopPropagation()
        }}
      >
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
          onClick={() => syncMail()}
        />
        <MenuItem
          icon={<Settings size={13} className="text-secondary" />}
          label={t('settings.label')}
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
      </div>
    </>
  )
}
