import { expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { firstDayOfWeek, monthGrid } from '../../lib/calendarGrid'
import { calendar$ } from '../../states/calendar'
import { CalendarView } from './CalendarView'

it('jumps to a date from the mini calendar even without a synced calendar', async () => {
  const month = new Date(2026, 10, 15)
  const target = monthGrid(month, firstDayOfWeek()).at(-1)!
  const monthTitle = new Intl.DateTimeFormat([], { month: 'long', year: 'numeric', calendar: 'gregory' })
  const dateLabel = new Intl.DateTimeFormat([], { dateStyle: 'full', calendar: 'gregory' }).format(target)
  const previous = calendar$.peek()
  const previousGo = Object.getOwnPropertyDescriptor(window, 'go')
  Object.defineProperty(window, 'go', {
    configurable: true,
    value: { main: { App: { Invoke: async () => ({}) } } },
  })
  calendar$.set({ ...previous, month: month.getTime(), day: month.getTime(), calendars: [], editor: null })
  try {
    await act(async () => {
      render(<CalendarView />)
    })
    const pane = within(screen.getByRole('complementary'))
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(monthTitle.format(month))
    await act(async () => fireEvent.click(pane.getByRole('button', { name: dateLabel })))
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(monthTitle.format(target))
    expect(pane.getByRole('button', { name: dateLabel }).getAttribute('aria-pressed')).toBe('true')
    expect(calendar$.editor.peek()).toBeNull()

    const trigger = pane.getByRole('button', { name: monthTitle.format(target) })
    fireEvent.click(trigger)
    let picker = within(pane.getByRole('dialog', { name: 'Jump to month' }))
    fireEvent.change(picker.getByLabelText('Year'), { target: { value: '' } })
    expect((picker.getByRole('button', { name: 'Select' }) as HTMLButtonElement).disabled).toBe(true)
    fireEvent.keyDown(picker.getByLabelText('Year'), { key: 'Escape' })
    expect(pane.queryByRole('dialog')).toBeNull()
    expect(document.activeElement).toBe(trigger)

    fireEvent.click(trigger)
    const todayButton = screen.getByRole('button', { name: 'Today' })
    expect(todayButton.closest('header')).toBe(screen.getByRole('heading', { level: 1 }).closest('header'))
    act(() => todayButton.focus())
    expect(pane.queryByRole('dialog')).toBeNull()
    expect(document.activeElement).toBe(todayButton)

    fireEvent.click(trigger)
    picker = within(pane.getByRole('dialog', { name: 'Jump to month' }))
    expect((picker.getByLabelText('Year') as HTMLInputElement).value).toBe(String(target.getFullYear()))
    fireEvent.click(picker.getByRole('button', { name: 'Previous year' }))
    expect((picker.getByLabelText('Year') as HTMLInputElement).value).toBe(String(target.getFullYear() - 1))
    fireEvent.click(picker.getByRole('button', { name: 'Next year' }))
    fireEvent.click(picker.getByRole('button', { name: 'Next year' }))
    fireEvent.change(picker.getByLabelText('Month'), { target: { value: '5' } })
    await act(async () => fireEvent.click(picker.getByRole('button', { name: 'Select' })))
    const nextYear = new Date(target.getFullYear() + 1, 5, 1)
    expect(calendar$.day.peek()).toBe(nextYear.getTime())
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(monthTitle.format(nextYear))
    expect(pane.queryByRole('dialog')).toBeNull()

    await act(async () => fireEvent.click(todayButton))
    expect(calendar$.day.peek()).toBe(new Date().setHours(0, 0, 0, 0))
  } finally {
    cleanup()
    calendar$.set(previous)
    if (previousGo) Object.defineProperty(window, 'go', previousGo)
    else delete (window as any).go
  }
})

it('imports and exports calendar files from the actions menu', async () => {
  const title = 'Import calendar file'
  const previous = calendar$.peek()
  const previousGo = Object.getOwnPropertyDescriptor(window, 'go')
  const calls: { method: string; payload: unknown }[] = []
  const summary = { imported: 2, duplicates: 1, skipped: 0 }
  const file = { path: '/tmp/fictional.ics', filename: 'fictional.ics', digest: 'preview-digest' }
  Object.defineProperty(window, 'go', {
    configurable: true,
    value: {
      main: {
        App: {
          Invoke: async (method: string, payload: unknown) => {
            calls.push({ method, payload })
            if (method === 'calendar.previewImport') return { ...summary, ...file }
            if (method === 'calendar.importIcs') return summary
            if (method === 'calendar.exportIcs') return { saved: true }
            return {}
          },
        },
      },
    },
  })
  calendar$.set({ ...previous, calendars: [], editor: null })
  try {
    await act(async () => {
      render(<CalendarView />)
    })
    fireEvent.click(screen.getByRole('button', { name: 'Calendar actions' }))
    const menu = within(screen.getByRole('dialog', { name: 'Calendar actions' }))
    await act(async () => fireEvent.click(menu.getByRole('button', { name: 'Import .ics…' })))
    const dialog = within(screen.getByRole('dialog', { name: title }))
    expect(dialog.getByText('fictional.ics')).toBeTruthy()
    expect(dialog.getByText(/2 events to import into/)).toBeTruthy()
    expect(calls.some(({ method }) => method === 'calendar.importIcs')).toBe(false)

    await act(async () => fireEvent.click(dialog.getByRole('button', { name: 'Import' })))
    expect(calls.find(({ method }) => method === 'calendar.importIcs')?.payload).toEqual({
      path: file.path,
      digest: file.digest,
    })
    expect(screen.queryByRole('dialog', { name: title })).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'Calendar actions' }))
    fireEvent.click(screen.getByRole('button', { name: 'Export calendar…' }))
    const exportDialog = within(screen.getByRole('dialog', { name: 'Export calendar' }))
    await act(async () => fireEvent.click(exportDialog.getByRole('button', { name: 'Export' })))
    expect(calls.find(({ method }) => method === 'calendar.exportIcs')?.payload).toEqual({ calendar: '' })
    expect(screen.queryByRole('dialog', { name: 'Export calendar' })).toBeNull()
  } finally {
    cleanup()
    calendar$.set(previous)
    if (previousGo) Object.defineProperty(window, 'go', previousGo)
    else delete (window as any).go
  }
})
