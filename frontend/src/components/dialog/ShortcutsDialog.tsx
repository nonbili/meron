import { useEffect } from 'react'
import { X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from 'react-i18next'
import { ui$ } from '../../states/ui'
import { formatShortcut, isMac, SHORTCUT_GROUPS, SHORTCUT_LABELS } from '../../lib/shortcuts'
import { IconButton } from '../button/IconButton'

/** Read-only cheat sheet listing every global shortcut, driven off the
 * SHORTCUTS table so it stays in sync automatically. Opened with ⌘/Ctrl+?. */
export function ShortcutsDialog() {
  const { t } = useTranslation()
  const open = useValue(ui$.shortcutsOpen)

  useEffect(() => {
    if (!open) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        ui$.shortcutsOpen.set(false)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [open])

  if (!open) return null

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) ui$.shortcutsOpen.set(false)
      }}
    >
      <div
        className="flex max-h-[80vh] w-full max-w-md flex-col overflow-hidden rounded-2xl border border-border bg-chats shadow-2xl"
        role="dialog"
        aria-modal="true"
        aria-label={t('shortcuts.title')}
      >
        <div className="flex items-center justify-between border-b border-border px-5 py-3.5">
          <h2 className="text-sm font-bold text-primary">{t('shortcuts.title')}</h2>
          <IconButton
            icon={X}
            iconSize={15}
            label={t('buttons.close')}
            size="sm"
            radius="lg"
            onClick={() => ui$.shortcutsOpen.set(false)}
          />
        </div>

        <div className="overflow-y-auto px-5 py-4 space-y-4">
          {SHORTCUT_GROUPS.map((group) => (
            <section key={group.title}>
              <h3 className="mb-1.5 text-[12px] font-semibold text-secondary">{group.title}</h3>
              <div className="overflow-hidden rounded-lg border border-border">
                {group.ids.map((id, i) => (
                  <div
                    key={id}
                    className={`flex items-center justify-between gap-4 px-3 py-2 text-[13px] text-primary ${
                      i > 0 ? 'border-t border-border' : ''
                    }`}
                  >
                    <span>{SHORTCUT_LABELS[id]}</span>
                    <kbd className="shrink-0 rounded border border-border bg-app px-1.5 py-0.5 text-[11px] font-medium text-secondary">
                      {formatShortcut(id).join(isMac ? '' : '+')}
                    </kbd>
                  </div>
                ))}
              </div>
            </section>
          ))}
        </div>
      </div>
    </div>
  )
}
