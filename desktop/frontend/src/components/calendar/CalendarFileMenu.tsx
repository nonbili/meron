import { useValue } from '@legendapp/state/react'
import { Download, MoreHorizontal, Upload } from 'lucide-react'
import { useRef, useState } from 'react'
import { invoke } from '../../lib/bridge'
import { errorMessage } from '../../lib/errors'
import { useTranslation } from '../../lib/i18n'
import { calendar$, loadCalendar } from '../../states/calendar'
import { showToast } from '../../states/ui'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { Dialog } from '../dialog/Dialog'
import { SelectInput } from '../field/Field'
import { MenuItem } from '../menu/MenuItem'
import { MenuSurface } from '../menu/MenuSurface'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'

type ImportSummary = { imported: number; duplicates: number; skipped: number }
type ImportPreview = ImportSummary & { path: string; filename: string; digest: string }
type Cancelled = { cancelled: true }

type FileAction = 'export' | ImportPreview

export function CalendarFileMenu() {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [picking, setPicking] = useState(false)
  const [action, setAction] = useState<FileAction | null>(null)
  const container = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const label = t('calendar.file.actions', { defaultValue: 'Calendar actions' })
  const importLabel = t('calendar.file.import', { defaultValue: 'Import .ics…' })
  const exportLabel = t('calendar.file.export', { defaultValue: 'Export calendar…' })

  useDismissOnOutside(
    open,
    (target) => container.current?.contains(target as Node) ?? false,
    () => {
      if (container.current?.contains(document.activeElement)) trigger.current?.focus()
      setOpen(false)
    },
  )

  const closeMenu = () => {
    trigger.current?.focus()
    setOpen(false)
  }

  const chooseFile = async () => {
    closeMenu()
    setPicking(true)
    try {
      const result = await invoke<ImportPreview | Cancelled>('calendar.previewImport')
      if (!('cancelled' in result)) setAction(result)
    } catch (err) {
      showToast(
        errorMessage(err, t('calendar.file.failed', { defaultValue: 'Could not transfer calendar events.' })),
        'error',
      )
    } finally {
      setPicking(false)
    }
  }

  return (
    <div ref={container} className="relative">
      <IconButton
        ref={trigger}
        label={label}
        icon={MoreHorizontal}
        size="sm"
        active={open}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-disabled={picking || undefined}
        onClick={() => {
          if (!picking) setOpen(!open)
        }}
      />
      {open && (
        <MenuSurface role="dialog" aria-label={label} className="absolute end-0 top-full z-50 mt-1 min-w-44 p-1">
          <MenuItem icon={<Upload size={14} />} label={importLabel} onClick={() => void chooseFile()} />
          <MenuItem
            icon={<Download size={14} />}
            label={exportLabel}
            onClick={() => {
              closeMenu()
              setAction('export')
            }}
          />
        </MenuSurface>
      )}
      {action && <CalendarFileDialog action={action} onClose={() => setAction(null)} />}
    </div>
  )
}

function CalendarFileDialog({ action, onClose }: { action: FileAction; onClose: () => void }) {
  const { t } = useTranslation()
  const calendars = useValue(calendar$.calendars)
  const [calendar, setCalendar] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  const run = async (operation: () => Promise<void>) => {
    setBusy(true)
    setError('')
    try {
      await operation()
    } catch (err) {
      setError(errorMessage(err, t('calendar.file.failed', { defaultValue: 'Could not transfer calendar events.' })))
    } finally {
      setBusy(false)
    }
  }

  const importFile = (file: ImportPreview) =>
    run(async () => {
      const summary = await invoke<ImportSummary>('calendar.importIcs', { path: file.path, digest: file.digest })
      void loadCalendar()
      showToast(
        t('calendar.file.imported', {
          defaultValue:
            'Imported {imported, plural, one {# event} other {# events}}. {duplicates, plural, one {# duplicate} other {# duplicates}} and {skipped, plural, one {# unsupported or invalid entry} other {# unsupported or invalid entries}} skipped.',
          ...summary,
        }),
        'success',
      )
      onClose()
    })

  const exportFile = () =>
    run(async () => {
      const result = await invoke<{ saved: boolean }>('calendar.exportIcs', { calendar })
      if (result.saved) {
        showToast(t('calendar.file.exported', { defaultValue: 'Calendar file saved.' }), 'success')
        onClose()
      }
    })

  const preview = action === 'export' ? null : action
  const title = preview
    ? t('calendar.file.importTitle', { defaultValue: 'Import calendar file' })
    : t('calendar.file.exportTitle', { defaultValue: 'Export calendar' })
  const localName = t('calendar.onThisDevice')
  return (
    <Dialog onClose={onClose} dismissible={!busy} aria-label={title} className="max-w-sm space-y-4 p-6">
      <h2 className="font-semibold text-primary">{title}</h2>
      {preview ? (
        <div className="space-y-2 text-sm">
          <p className="break-all font-medium text-primary">{preview.filename}</p>
          <p className="text-secondary">
            {t('calendar.file.preview', {
              defaultValue:
                '{imported, plural, one {# event} other {# events}} to import into {calendar}. {duplicates, plural, one {# duplicate} other {# duplicates}} and {skipped, plural, one {# unsupported or invalid entry} other {# unsupported or invalid entries}} will be skipped.',
              imported: preview.imported,
              duplicates: preview.duplicates,
              skipped: preview.skipped,
              calendar: localName,
            })}
          </p>
        </div>
      ) : (
        <SelectInput
          aria-label={t('calendar.calendars')}
          value={calendar}
          disabled={busy}
          onChange={(event) => setCalendar(event.target.value)}
        >
          <option value="">{localName}</option>
          {calendars.map(({ id, name, account_name }) => (
            <option key={id} value={id}>
              {name} ({account_name})
            </option>
          ))}
        </SelectInput>
      )}
      {error && (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">
          {error}
        </p>
      )}
      <div className="flex justify-end gap-2 border-t border-border pt-3">
        <Button variant="secondary" size="sm" disabled={busy} onClick={onClose}>
          {t('buttons.cancel')}
        </Button>
        {preview ? (
          <Button size="sm" disabled={busy || preview.imported === 0} onClick={() => void importFile(preview)}>
            {t('common.import')}
          </Button>
        ) : (
          <Button size="sm" disabled={busy} onClick={() => void exportFile()}>
            {t('common.export')}
          </Button>
        )}
      </div>
    </Dialog>
  )
}
