import { afterEach, beforeEach, describe, expect, it } from 'bun:test'

import { ui$ } from './ui'
import { setCalendarEnabled } from './settings'
import {
  calendar$,
  closeCalendar,
  closeCalendarPanel,
  openCalendar,
  openCalendarPanel,
  restoreCalendarSession,
  selectDay,
  shiftMonth,
  toggleCalendarPanel,
  calendarColor,
  visibleItems,
  LOCAL_CALENDAR,
  type CalendarItem,
} from './calendar'
import { openTasksPanel } from './tasks'

const calls: { command: string; payload: any }[] = []
const originalMatchMedia = (window as any).matchMedia
let narrow = false

beforeEach(() => {
  calls.length = 0
  narrow = false
  ui$.calendarOpen.set(false)
  ui$.calendarPanelOpen.set(false)
  ui$.tasksPanelOpen.set(false)
  calendar$.editor.set(null)
  ;(window as any).matchMedia = () => ({ matches: narrow })
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push({ command, payload })
          if (command === 'tasks.lists')
            return { lists: [{ id: 'list-1', title: 'My Tasks' }], default_list_id: 'list-1' }
          if (command === 'tasks.items') return { tasks: [] }
          if (command === 'calendar.events') return { events: [] }
          return { ok: true }
        },
      },
    },
  }
})

afterEach(() => {
  ;(window as any).matchMedia = originalMatchMedia
})

describe('toggleCalendarPanel', () => {
  it('opens the panel, and closes it again', () => {
    toggleCalendarPanel()
    expect(ui$.calendarPanelOpen.get()).toBe(true)
    expect(ui$.calendarOpen.get()).toBe(false)
    toggleCalendarPanel()
    expect(ui$.calendarPanelOpen.get()).toBe(false)
  })

  it('collapses the full view into the panel', () => {
    openCalendar()
    toggleCalendarPanel()
    expect(ui$.calendarOpen.get()).toBe(false)
    expect(ui$.calendarPanelOpen.get()).toBe(true)
  })

  it('toggles the full view on a window too narrow for the panel', () => {
    narrow = true
    toggleCalendarPanel()
    expect(ui$.calendarOpen.get()).toBe(true)
    expect(ui$.calendarPanelOpen.get()).toBe(false)
    toggleCalendarPanel()
    expect(ui$.calendarOpen.get()).toBe(false)
  })

  it('opens the full view over a panel the window has since hidden', () => {
    openCalendarPanel()
    narrow = true
    toggleCalendarPanel()
    expect(ui$.calendarOpen.get()).toBe(true)
  })
})

describe('the right-hand slot', () => {
  it('hands the slot from Tasks to the calendar and back', async () => {
    await openTasksPanel()
    openCalendarPanel()
    expect(ui$.tasksPanelOpen.get()).toBe(false)
    expect(ui$.calendarPanelOpen.get()).toBe(true)

    calendar$.editor.set({ mode: 'new', day: Date.now() })
    await openTasksPanel()
    expect(ui$.calendarPanelOpen.get()).toBe(false)
    expect(ui$.tasksPanelOpen.get()).toBe(true)
    expect(calendar$.editor.get()).toBeNull()
  })

  it('keeps the panel open under the full view, for when it gives way', () => {
    openCalendarPanel()
    openCalendar()
    expect(ui$.calendarOpen.get()).toBe(true)
    expect(ui$.calendarPanelOpen.get()).toBe(true)
    // Opening a board or an account clears it directly.
    ui$.calendarOpen.set(false)
    expect(ui$.calendarPanelOpen.get()).toBe(true)
  })

  it('goes with the calendar setting', () => {
    openCalendarPanel()
    setCalendarEnabled(false)
    expect(ui$.calendarPanelOpen.get()).toBe(false)
    setCalendarEnabled(true)
  })
})

describe('the agenda day', () => {
  it('pages the month to a neighbouring month day picked in the grid', () => {
    calendar$.month.set(new Date(2026, 9, 15).getTime())
    selectDay(new Date(2026, 10, 2, 13, 30))
    expect(calendar$.day.get()).toBe(new Date(2026, 10, 2).getTime())
    expect(new Date(calendar$.month.get()).getMonth()).toBe(10)
  })

  it('asks for the week ahead even where it runs past the month grid', async () => {
    calendar$.month.set(new Date(2026, 9, 15).getTime())
    selectDay(new Date(2026, 10, 7))
    await Promise.resolve()
    const asked = calls.filter((call) => call.command === 'calendar.events').at(-1)!.payload
    expect(asked.to * 1000).toBeGreaterThanOrEqual(new Date(2026, 10, 14).getTime())
  })

  it('moves to the first of a month paged to', () => {
    calendar$.month.set(new Date(2026, 0, 15).getTime())
    shiftMonth(1)
    expect(calendar$.day.get()).toBe(new Date(2026, 1, 1).getTime())
  })
})

describe('restoreCalendarSession', () => {
  it('reopens a panel left open', () => {
    restoreCalendarSession({ session_calendar_panel: true })
    expect(ui$.calendarPanelOpen.get()).toBe(true)
    closeCalendarPanel()
  })

  it('keeps it shut with the calendar switched off', () => {
    restoreCalendarSession({ calendar_enabled: false, session_calendar_panel: true })
    expect(ui$.calendarPanelOpen.get()).toBe(false)
  })
})

describe('openCalendarPanel', () => {
  it('collapses the full view back into the panel', () => {
    openCalendarPanel()
    openCalendar()
    openCalendarPanel()
    expect(ui$.calendarOpen.get()).toBe(false)
    expect(ui$.calendarPanelOpen.get()).toBe(true)
  })
})

describe('the event editor', () => {
  it('closes when a board or an account takes over from the full view', () => {
    openCalendar()
    calendar$.editor.set({ mode: 'new', day: Date.now() })
    ui$.calendarOpen.set(false)
    expect(calendar$.editor.get()).toBeNull()
  })

  it('closes with the panel, however it goes', () => {
    openCalendarPanel()
    calendar$.editor.set({ mode: 'new', day: Date.now() })
    setCalendarEnabled(false)
    expect(calendar$.editor.get()).toBeNull()
    setCalendarEnabled(true)
  })
})

describe('calendar visibility', () => {
  const item = (source: CalendarItem['source'], calendar = ''): CalendarItem =>
    ({ id: `${source}-${calendar}`, key: `${source}-${calendar}`, source, calendar }) as CalendarItem

  it('hides the events of hidden calendars, never tasks', () => {
    const items = [item('event'), item('invite'), item('event', 'cal-work'), item('task')]
    expect(visibleItems(items, []).length).toBe(4)
    expect(visibleItems(items, [LOCAL_CALENDAR]).map((i) => i.id)).toEqual(['event-cal-work', 'task-'])
    expect(visibleItems(items, ['cal-work']).map((i) => i.id)).toEqual(['event-', 'invite-', 'task-'])
  })

  it('a calendar without a colour gets a steady preset', () => {
    expect(calendarColor({ id: 'cal-1', color: '#0082C9' })).toBe('#0082c9')
    const picked = calendarColor({ id: 'cal-1', color: '' })
    expect(picked).toMatch(/^#[0-9a-f]{6}$/)
    expect(calendarColor({ id: 'cal-1', color: '' })).toBe(picked)
  })
})
