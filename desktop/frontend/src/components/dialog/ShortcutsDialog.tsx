import { useEffect, useState } from 'react'
import { RotateCcw, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { ui$ } from '../../states/ui'
import {
  chordFromEvent,
  formatShortcut,
  isMac,
  SHORTCUT_GROUPS,
  SHORTCUT_LABELS,
  shortcutConflict,
  type ShortcutId,
} from '../../lib/shortcuts'
import { resetAllShortcutBindings, resetShortcutBinding, setShortcutBinding, settings$ } from '../../states/settings'
import { IconButton } from '../button/IconButton'

/** Cheat sheet listing every global shortcut, driven off the shortcut table so
 * it stays in sync automatically. Each row is also the editor: click it and the
 * next keystroke becomes that shortcut's chord. Opened with ⌘/Ctrl+?. */
export function ShortcutsDialog() {
  const { t } = useTranslation()
  const open = useValue(ui$.shortcutsOpen)
  // Subscribes the whole sheet to rebindings, so every row re-renders after an
  // edit or a reset (formatShortcut itself reads a plain module mirror).
  const overrides = useValue(settings$.shortcutOverrides)
  // The shortcut currently listening for a keystroke, if any.
  const [recording, setRecording] = useState<ShortcutId | null>(null)
  // Which shortcut already owns the chord the user just pressed.
  const [conflict, setConflict] = useState<{ id: ShortcutId; taken: ShortcutId } | null>(null)

  // Esc cancels a recording, else closes the sheet. useEscapeKey hands the key
  // to the topmost layer only, so Settings underneath does not close too.
  useEscapeKey(() => {
    if (recording) {
      setRecording(null)
      setConflict(null)
    } else {
      ui$.shortcutsOpen.set(false)
    }
  }, open)

  // The recorder runs in the capture phase and swallows the keystroke, so a
  // chord being rebound (⌘K, say) doesn't also trigger its current action.
  useEffect(() => {
    if (!recording) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') return
      const chord = chordFromEvent(event)
      if (!chord) return
      event.preventDefault()
      event.stopPropagation()
      const taken = shortcutConflict(recording, chord)
      if (taken) {
        setConflict({ id: recording, taken })
        return
      }
      setShortcutBinding(recording, chord)
      setConflict(null)
      setRecording(null)
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [recording])

  useEffect(() => {
    if (open) return
    setRecording(null)
    setConflict(null)
  }, [open])

  if (!open) return null

  const startRecording = (id: ShortcutId) => {
    setConflict(null)
    setRecording((current) => (current === id ? null : id))
  }

  const customized = Object.keys(overrides).length > 0

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
          <div className="flex items-center gap-1">
            {customized && (
              <button
                type="button"
                className="rounded-lg px-2 py-1 text-[0.75rem] font-medium text-secondary hover:bg-app hover:text-primary"
                onClick={() => {
                  resetAllShortcutBindings()
                  setRecording(null)
                  setConflict(null)
                }}
              >
                {t('shortcuts.resetAll')}
              </button>
            )}
            <IconButton
              icon={X}
              iconSize={15}
              label={t('buttons.close')}
              size="sm"
              radius="lg"
              onClick={() => ui$.shortcutsOpen.set(false)}
            />
          </div>
        </div>

        <div className="overflow-y-auto px-5 py-4 space-y-4">
          <p className="text-[0.75rem] text-secondary">{t('shortcuts.customizeHint')}</p>
          {SHORTCUT_GROUPS.map((group) => (
            <section key={group.title}>
              <h3 className="mb-1.5 text-[0.75rem] font-semibold text-secondary">{group.title}</h3>
              <div className="overflow-hidden rounded-lg border border-border">
                {group.ids.map((id, i) => (
                  <ShortcutRow
                    key={id}
                    id={id}
                    first={i === 0}
                    recording={recording === id}
                    conflict={conflict?.id === id ? conflict.taken : null}
                    customized={!!overrides[id]}
                    onEdit={() => startRecording(id)}
                    onReset={() => {
                      resetShortcutBinding(id)
                      if (recording === id) setRecording(null)
                      setConflict(null)
                    }}
                  />
                ))}
              </div>
            </section>
          ))}
        </div>
      </div>
    </div>
  )
}

function ShortcutRow({
  id,
  first,
  recording,
  conflict,
  customized,
  onEdit,
  onReset,
}: {
  id: ShortcutId
  first: boolean
  recording: boolean
  conflict: ShortcutId | null
  customized: boolean
  onEdit: () => void
  onReset: () => void
}) {
  const { t } = useTranslation()

  return (
    <div className={`px-3 py-2 text-[0.8125rem] text-primary ${first ? '' : 'border-t border-border'}`}>
      <div className="flex items-center justify-between gap-4">
        <span>{SHORTCUT_LABELS[id]}</span>
        <div className="flex shrink-0 items-center gap-1">
          {customized && (
            <IconButton
              icon={RotateCcw}
              iconSize={13}
              label={t('shortcuts.resetOne')}
              size="sm"
              radius="lg"
              onClick={onReset}
            />
          )}
          <button
            type="button"
            aria-label={t('shortcuts.rebind')}
            className={`rounded border px-1.5 py-0.5 text-[0.6875rem] font-medium ${
              recording
                ? 'border-accent text-accent'
                : 'border-border bg-app text-secondary hover:border-accent hover:text-primary'
            }`}
            onClick={onEdit}
          >
            {recording ? t('shortcuts.pressKeys') : formatShortcut(id).join(isMac ? '' : '+')}
          </button>
        </div>
      </div>
      {conflict && (
        <p className="mt-1 text-right text-[0.6875rem] text-rose-600 dark:text-rose-400">
          {t('shortcuts.conflict', { name: SHORTCUT_LABELS[conflict] })}
        </p>
      )}
    </div>
  )
}
