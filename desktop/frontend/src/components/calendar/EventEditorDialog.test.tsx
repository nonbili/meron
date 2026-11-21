import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { dateValue, timeValue } from '../../lib/calendarGrid'
import { calendar$ } from '../../states/calendar'
import { EventEditorDialog } from './EventEditorDialog'

describe('custom repeat', () => {
  let calls: { method: string; payload: any }[]
  let previousGo: PropertyDescriptor | undefined

  beforeEach(() => {
    calls = []
    calendar$.calendars.set([])
    previousGo = Object.getOwnPropertyDescriptor(window, 'go')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (method: string, payload: unknown) => {
            calls.push({ method, payload })
            return {}
          },
        },
      },
    }
  })

  afterEach(() => {
    cleanup()
    calendar$.editor.set(null)
    if (previousGo) Object.defineProperty(window, 'go', previousGo)
    else delete (window as any).go
  })

  it('makes a rule from the event’s day and saves it only with the event', async () => {
    // Friday 20 November 2026.
    render(<EventEditorDialog editor={{ mode: 'new', day: new Date(2026, 10, 20).getTime() }} />)
    const editor = within(screen.getByRole('dialog'))
    fireEvent.change(editor.getByRole('textbox', { name: 'Title' }), { target: { value: 'Review' } })
    const repeat = editor.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    const customize = within(repeat).getByRole<HTMLOptionElement>('option', { name: 'Custom…' })
    fireEvent.change(repeat, { target: { value: customize.value } })

    const custom = within(screen.getByRole('dialog', { name: 'Custom repeat' }))
    // Weekly on the event's own weekday to begin with.
    expect(custom.getByRole('button', { name: 'Friday' }).getAttribute('aria-pressed')).toBe('true')
    expect(custom.getByRole('button', { name: 'Monday' }).getAttribute('aria-pressed')).toBe('false')
    fireEvent.change(custom.getByRole('spinbutton', { name: 'Repeat every' }), { target: { value: '2' } })
    fireEvent.click(custom.getByRole('button', { name: 'Monday' }))
    fireEvent.click(custom.getByRole('radio', { name: 'After' }))
    fireEvent.change(custom.getByRole('spinbutton', { name: 'After' }), { target: { value: '5' } })
    expect(custom.getByRole('status').textContent).toBe('Every 2 weeks on Mon, Fri, 5 times')

    // The series' own first day stays one of them.
    expect(custom.getByRole<HTMLButtonElement>('button', { name: 'Friday' }).disabled).toBe(true)

    // Clearing the end date while it is chosen leaves the summary, and the
    // dialog, standing.
    fireEvent.click(custom.getByRole('radio', { name: 'On' }))
    fireEvent.change(custom.getByLabelText('On', { selector: 'input[type=date]' }), { target: { value: '' } })
    fireEvent.click(custom.getByRole('radio', { name: 'Never' }))
    fireEvent.click(custom.getByRole('radio', { name: 'On' }))
    expect(custom.getByRole<HTMLButtonElement>('button', { name: 'Done' }).disabled).toBe(true)
    expect(custom.getByRole('status').textContent).toBe('Every 2 weeks on Mon, Fri')
    fireEvent.click(custom.getByRole('radio', { name: 'After' }))

    await act(async () => fireEvent.click(custom.getByRole('button', { name: 'Done' })))
    expect(screen.queryByRole('dialog', { name: 'Custom repeat' })).toBeNull()
    expect(calls.some(({ method }) => method === 'calendar.create')).toBe(false)
    // Its weeks start on Sunday, as the picker showed them in English.
    expect(repeat.value).toBe('FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;WKST=SU;COUNT=5')
    expect(repeat.selectedOptions[0].textContent).toBe('Every 2 weeks on Mon, Fri, 5 times')

    // Moved from Friday to Tuesday, the days move with it, and the weeks too.
    fireEvent.change(editor.getByLabelText('Starts', { selector: 'input[type=date]' }), {
      target: { value: '2026-11-24' },
    })
    expect(repeat.value).toBe('FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,FR;WKST=TH;COUNT=5')

    await act(async () => fireEvent.click(editor.getByRole('button', { name: 'Save' })))
    const create = calls.find(({ method }) => method === 'calendar.create')
    expect(create?.payload.repeat).toBe('FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,FR;WKST=TH;COUNT=5')
  })

  it('closes on Escape without touching the event editor or its repeat', async () => {
    render(<EventEditorDialog editor={{ mode: 'new', day: new Date(2026, 10, 20).getTime() }} />)
    const repeat = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    fireEvent.change(repeat, {
      target: { value: within(repeat).getByRole<HTMLOptionElement>('option', { name: 'Custom…' }).value },
    })
    expect(screen.getByRole('dialog', { name: 'Custom repeat' })).toBeTruthy()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByRole('dialog', { name: 'Custom repeat' })).toBeNull()
    expect(screen.getAllByRole('dialog')).toHaveLength(1)
    expect(repeat.value).toBe('')
  })

  it('names an event’s own rule, and one only its server can keep', () => {
    const item = {
      id: 'e1',
      key: 'e1@1',
      source: 'event' as const,
      title: 'Review',
      notes: '',
      location: '',
      start: Math.floor(new Date(2026, 10, 20, 9).getTime() / 1000),
      end: Math.floor(new Date(2026, 10, 20, 10).getTime() / 1000),
      all_day: false,
      recurring: true,
      repeat: 'FREQ=MONTHLY;BYDAY=3FR',
      account: '',
      thread_id: '',
      message_id: '',
      response: '',
    }
    render(<EventEditorDialog editor={{ mode: 'edit', item }} />)
    const repeat = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    expect(repeat.selectedOptions[0].textContent).toBe('Every month on the third Friday')
    cleanup()

    calendar$.calendars.set([
      { id: 'cal', name: 'Work', color: '', writable: true, account_id: 'dav', account_name: 'Work account' },
    ])
    render(<EventEditorDialog editor={{ mode: 'edit', item: { ...item, repeat: '' } }} />)
    const local = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Calendar' })
    expect(local.selectedOptions[0].textContent).toBe('On this device')
    fireEvent.change(local, { target: { value: 'cal' } })
    expect(within(local).getByRole('option', { name: 'On this device' })).toBeTruthy()
    cleanup()

    render(<EventEditorDialog editor={{ mode: 'edit', item: { ...item, repeat: '', calendar: 'cal' } }} />)
    const server = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    expect(server.selectedOptions[0].textContent).toBe('Set by the calendar server')
  })

  it('counts a custom repeat from the series’ own first day, not the occurrence open', () => {
    // A November occurrence of a series that began on Monday 5 October.
    const start = Math.floor(new Date(2026, 10, 23, 9).getTime() / 1000)
    const item = {
      id: 'e1',
      key: 'e1@2',
      source: 'event' as const,
      title: 'Review',
      notes: '',
      location: '',
      start,
      end: start + 3600,
      all_day: false,
      recurring: true,
      repeat: 'monthly',
      series_date: '2026-10-05',
      account: '',
      thread_id: '',
      message_id: '',
      response: '',
    }
    render(<EventEditorDialog editor={{ mode: 'edit', item }} />)
    const repeat = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    fireEvent.change(repeat, {
      target: { value: within(repeat).getByRole<HTMLOptionElement>('option', { name: 'Custom…' }).value },
    })
    const custom = within(screen.getByRole('dialog', { name: 'Custom repeat' }))
    const monthly = custom.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat on' })
    expect([...monthly.options].map((option) => option.textContent)).toEqual([
      'Monthly on day 5',
      'Monthly on the first Monday',
    ])
    fireEvent.click(custom.getByRole('radio', { name: 'On' }))
    expect(custom.getByLabelText<HTMLInputElement>('On', { selector: 'input[type=date]' }).min).toBe('2026-10-05')
  })

  it('moves an existing series’ rule with its start, as far as it moves in the series’ zone', async () => {
    // Monday 23:00 UTC, every Monday and Wednesday.
    const start = Date.UTC(2026, 10, 23, 23) / 1000
    const item = {
      id: 'e1',
      key: 'e1@1',
      source: 'event' as const,
      title: 'Review',
      notes: '',
      location: '',
      start,
      end: start + 3600,
      all_day: false,
      recurring: true,
      repeat: 'FREQ=WEEKLY;BYDAY=MO,WE',
      series_date: '2026-11-23',
      series_zone: 'UTC',
      account: '',
      thread_id: '',
      message_id: '',
      response: '',
    }
    render(<EventEditorDialog editor={{ mode: 'edit', item }} />)
    // Two hours later, wherever this device is: Tuesday in UTC.
    const later = new Date((start + 2 * 3600) * 1000)
    fireEvent.change(screen.getByLabelText('Starts', { selector: 'input[type=date]' }), {
      target: { value: dateValue(later) },
    })
    fireEvent.change(screen.getByLabelText('Start time'), { target: { value: timeValue(later) } })
    const repeat = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    expect(repeat.value).toBe('FREQ=WEEKLY;BYDAY=TU,TH')
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Save' })))
    const update = calls.find(({ method }) => method === 'calendar.update')
    expect(update?.payload.repeat).toBe('FREQ=WEEKLY;BYDAY=TU,TH')
  })

  it('counts a repeat chosen after a move from where the series now starts', async () => {
    const start = Math.floor(new Date(2026, 10, 23, 9).getTime() / 1000)
    const item = {
      id: 'e1',
      key: 'e1@1',
      source: 'event' as const,
      title: 'Review',
      notes: '',
      location: '',
      start,
      end: start + 3600,
      all_day: false,
      recurring: true,
      repeat: 'weekly',
      series_date: '2026-11-23',
      account: '',
      thread_id: '',
      message_id: '',
      response: '',
    }
    render(<EventEditorDialog editor={{ mode: 'edit', item }} />)
    // Monday to Tuesday, then a rule picked there.
    fireEvent.change(screen.getByLabelText('Starts', { selector: 'input[type=date]' }), {
      target: { value: '2026-11-24' },
    })
    const repeat = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    fireEvent.change(repeat, {
      target: { value: within(repeat).getByRole<HTMLOptionElement>('option', { name: 'Custom…' }).value },
    })
    const custom = within(screen.getByRole('dialog', { name: 'Custom repeat' }))
    expect(custom.getByRole<HTMLButtonElement>('button', { name: 'Tuesday' }).disabled).toBe(true)
    fireEvent.click(custom.getByRole('button', { name: 'Friday' }))
    await act(async () => fireEvent.click(custom.getByRole('button', { name: 'Done' })))
    expect(repeat.value).toBe('FREQ=WEEKLY;BYDAY=TU,FR')
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Save' })))
    const update = calls.find(({ method }) => method === 'calendar.update')
    // Different from the repeat it opened with, so the core takes it as it is.
    expect(update?.payload.repeat).toBe('FREQ=WEEKLY;BYDAY=TU,FR')
    expect(update?.payload.original.repeat).toBe('weekly')
  })

  it('restores a rule exactly when the start comes back', () => {
    // The last Friday of the month, from Friday 27 November.
    const start = Math.floor(new Date(2026, 10, 27, 9).getTime() / 1000)
    const item = {
      id: 'e1',
      key: 'e1@1',
      source: 'event' as const,
      title: 'Review',
      notes: '',
      location: '',
      start,
      end: start + 3600,
      all_day: false,
      recurring: true,
      repeat: 'FREQ=MONTHLY;BYDAY=-1FR',
      series_date: '2026-11-27',
      series_zone: '',
      account: '',
      thread_id: '',
      message_id: '',
      response: '',
    }
    render(<EventEditorDialog editor={{ mode: 'edit', item }} />)
    const starts = screen.getByLabelText('Starts', { selector: 'input[type=date]' })
    const repeat = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    fireEvent.change(starts, { target: { value: '2026-11-20' } })
    expect(repeat.value).toBe('FREQ=MONTHLY;BYDAY=3FR')
    fireEvent.change(starts, { target: { value: '2026-11-27' } })
    expect(repeat.value).toBe('FREQ=MONTHLY;BYDAY=-1FR')
  })

  it('moves a rule with a series made all-day on the date shown', () => {
    // Monday 23:00 UTC, every Monday and Wednesday.
    const start = Date.UTC(2026, 10, 23, 23) / 1000
    const item = {
      id: 'e1',
      key: 'e1@1',
      source: 'event' as const,
      title: 'Review',
      notes: '',
      location: '',
      start,
      end: start + 3600,
      all_day: false,
      recurring: true,
      repeat: 'FREQ=WEEKLY;BYDAY=MO,WE',
      series_date: '2026-11-23',
      series_zone: 'UTC',
      account: '',
      thread_id: '',
      message_id: '',
      response: '',
    }
    render(<EventEditorDialog editor={{ mode: 'edit', item }} />)
    fireEvent.click(screen.getByRole('checkbox', { name: 'All day' }))
    // All day on the date this device shows, which is Tuesday east of UTC.
    const shown = dateValue(new Date(start * 1000))
    const repeat = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    expect(repeat.value).toBe(shown === '2026-11-23' ? 'FREQ=WEEKLY;BYDAY=MO,WE' : 'FREQ=WEEKLY;BYDAY=TU,TH')
  })

  it('offers an event’s own rule only while its start has not moved', () => {
    const start = Math.floor(new Date(2026, 10, 23, 9).getTime() / 1000)
    const item = {
      id: 'e1',
      key: 'e1@1',
      source: 'event' as const,
      title: 'Review',
      notes: '',
      location: '',
      start,
      end: start + 3600,
      all_day: false,
      recurring: true,
      repeat: 'FREQ=WEEKLY;BYDAY=MO;COUNT=4',
      series_date: '2026-11-23',
      series_zone: '',
      account: '',
      thread_id: '',
      message_id: '',
      response: '',
    }
    render(<EventEditorDialog editor={{ mode: 'edit', item }} />)
    const repeat = screen.getByRole<HTMLSelectElement>('combobox', { name: 'Repeat' })
    const rules = () => [...repeat.options].map((option) => option.value).filter((value) => value.startsWith('FREQ='))
    expect(rules()).toEqual(['FREQ=WEEKLY;BYDAY=MO;COUNT=4'])
    fireEvent.change(screen.getByLabelText('Starts', { selector: 'input[type=date]' }), {
      target: { value: '2026-11-24' },
    })
    expect(rules()).toEqual(['FREQ=WEEKLY;BYDAY=TU;COUNT=4'])
  })
})
