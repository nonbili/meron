import { describe, expect, it } from 'bun:test'
import { droppedEventStart, gridRange, localInstant, monthGrid, occursOn, utcDate, utcDateValue } from './calendarGrid'

describe('monthGrid', () => {
  it('shows six weeks from the week the month starts in', () => {
    // October 2026 starts on a Thursday.
    const sunday = monthGrid(new Date(2026, 9, 15), 0)
    expect(sunday).toHaveLength(42)
    expect(sunday[0].getDay()).toBe(0)
    expect(sunday[0].getDate()).toBe(27)
    expect(sunday[4].getDate()).toBe(1)
    const monday = monthGrid(new Date(2026, 9, 15), 1)
    expect(monday[0].getDay()).toBe(1)
    expect(monday[3].getDate()).toBe(1)
  })

  it('covers its days with a day to spare either side', () => {
    const days = monthGrid(new Date(2026, 9, 1))
    const { from, to } = gridRange(days)
    expect(from).toBe(days[0].getTime() / 1000 - 86_400)
    expect(to).toBe(new Date(2026, 10, 8).getTime() / 1000 + 86_400)
  })
})

describe('occursOn', () => {
  it('places an all-day item on its UTC dates, end exclusive', () => {
    const item = { start: utcDate('2026-10-05'), end: utcDate('2026-10-07'), all_day: true }
    expect(occursOn(item, new Date(2026, 9, 4))).toBe(false)
    expect(occursOn(item, new Date(2026, 9, 5))).toBe(true)
    expect(occursOn(item, new Date(2026, 9, 6))).toBe(true)
    expect(occursOn(item, new Date(2026, 9, 7))).toBe(false)
    expect(utcDateValue(item.start)).toBe('2026-10-05')
  })

  it('places a timed item on every local day it touches', () => {
    const start = localInstant('2026-10-05', '22:00')
    const overnight = { start, end: start + 4 * 3600, all_day: false }
    expect(occursOn(overnight, new Date(2026, 9, 5))).toBe(true)
    expect(occursOn(overnight, new Date(2026, 9, 6))).toBe(true)
    expect(occursOn(overnight, new Date(2026, 9, 7))).toBe(false)
    // Ending at midnight does not reach the next day; an instant has one day.
    const toMidnight = { start, end: localInstant('2026-10-06', '00:00'), all_day: false }
    expect(occursOn(toMidnight, new Date(2026, 9, 6))).toBe(false)
    const instant = { start, end: start, all_day: false }
    expect(occursOn(instant, new Date(2026, 9, 5))).toBe(true)
    expect(occursOn(instant, new Date(2026, 9, 6))).toBe(false)
  })
})

describe('droppedEventStart', () => {
  it('moves a multi-day all-day event from the date grabbed across a month boundary', () => {
    const event = { start: utcDate('2026-10-30'), end: utcDate('2026-11-02'), all_day: true }
    expect(droppedEventStart(event, new Date(2026, 9, 31), new Date(2026, 10, 3))).toBe(utcDate('2026-11-02'))
  })

  it('keeps the continuation of an overnight event under the dropped hour', () => {
    const start = localInstant('2026-10-05', '22:00')
    const event = { start, end: localInstant('2026-10-06', '02:00'), all_day: false }
    expect(droppedEventStart(event, new Date(2026, 9, 6), new Date(2026, 9, 8), 9)).toBe(
      localInstant('2026-10-08', '07:00'),
    )
  })
})
