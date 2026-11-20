import { describe, expect, it } from 'bun:test'
import { alignRepeat, describeRepeat, formatRepeat, initialRule, parseRepeat, weekdayPlace } from './repeatRule'

// Echo keys and values, so a summary's shape can be checked without a catalog.
const t = (key: string, values: Record<string, unknown> = {}) =>
  `${key.replace('calendar.repeat.', '')}(${Object.entries(values)
    .map(([name, value]) => `${name}=${value}`)
    .join(',')})`

describe('repeat rules', () => {
  it('keep the canonical form the core stores, and a plain choice by name', () => {
    for (const stored of [
      'weekly',
      'FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE',
      'FREQ=MONTHLY;BYDAY=-1FR;COUNT=6',
      'FREQ=MONTHLY;BYMONTHDAY=20;UNTIL=20270630',
      'FREQ=YEARLY;INTERVAL=3',
      'FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH;WKST=SU',
    ]) {
      expect(formatRepeat(parseRepeat(stored)!)).toBe(stored)
    }
    expect(formatRepeat(parseRepeat('FREQ=DAILY')!)).toBe('daily')
    expect(formatRepeat(parseRepeat('FREQ=WEEKLY;BYDAY=WE,MO,WE')!)).toBe('FREQ=WEEKLY;BYDAY=MO,WE')
    // A week start is kept only where it picks the weeks, as the core keeps it.
    expect(formatRepeat(parseRepeat('FREQ=WEEKLY;BYDAY=TU,TH;WKST=SU')!)).toBe('FREQ=WEEKLY;BYDAY=TU,TH')
  })

  it('leave out what the editor cannot say', () => {
    expect(parseRepeat('')).toBeNull()
    expect(parseRepeat('custom')).toBeNull()
    expect(parseRepeat('FREQ=HOURLY')).toBeNull()
    expect(parseRepeat('FREQ=DAILY;BYDAY=MO')).toBeNull()
    expect(parseRepeat('FREQ=MONTHLY;BYDAY=MO,TU;BYSETPOS=-1')).toBeNull()
  })

  it('start the custom editor from the event’s own day', () => {
    // Friday 20 November 2026: the third Friday, not the last.
    const day = new Date(2026, 10, 20)
    expect(initialRule('', day)).toMatchObject({ freq: 'weekly', weekdays: [4] })
    expect(initialRule('monthly', day).monthly).toEqual({ kind: 'day', day: 20 })
    expect(weekdayPlace(day)).toEqual({ nth: 3, last: false })
    expect(weekdayPlace(new Date(2026, 10, 27))).toEqual({ nth: 4, last: true })
  })

  it('describe each part in words', () => {
    const rule = parseRepeat('FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;UNTIL=20270630')!
    expect(describeRepeat(rule, t, 'en')).toBe(
      'summaryUntil(rule=summaryOnDays(rule=everyWeeks(count=2),days=Mon, Wed),date=Jun 30, 2027)',
    )
    expect(describeRepeat(parseRepeat('FREQ=MONTHLY;BYDAY=5FR')!, t, 'en')).toContain('nth=nth5()')
    // An end date still being typed is left out rather than formatted.
    expect(describeRepeat({ ...rule, end: { kind: 'until', date: '' } }, t, 'en')).not.toContain('summaryUntil')
    expect(describeRepeat(parseRepeat('FREQ=MONTHLY;BYDAY=-1FR;COUNT=6')!, t, 'en')).toBe(
      'summaryCount(rule=summaryOnWeekday(rule=everyMonths(count=1),nth=nthLast(),weekday=Friday),count=6)',
    )
  })

  it('move with their series, as the core moves them', () => {
    const friday = new Date(2026, 10, 20)
    const tuesday = new Date(2026, 10, 24)
    expect(alignRepeat('FREQ=WEEKLY;BYDAY=MO,FR;COUNT=4', friday, tuesday)).toBe('FREQ=WEEKLY;BYDAY=TU,FR;COUNT=4')
    expect(alignRepeat('FREQ=WEEKLY;BYDAY=TU,FR', friday, tuesday)).toBe('FREQ=WEEKLY;BYDAY=TU,SA')
    expect(alignRepeat('FREQ=MONTHLY;BYMONTHDAY=20', friday, tuesday)).toBe('FREQ=MONTHLY;BYMONTHDAY=24')
    expect(alignRepeat('FREQ=MONTHLY;BYDAY=3FR', friday, tuesday)).toBe('FREQ=MONTHLY;BYDAY=4TU')
    expect(alignRepeat('FREQ=MONTHLY;BYDAY=-1FR', new Date(2026, 10, 27), tuesday)).toBe('FREQ=MONTHLY;BYDAY=-1TU')
    expect(alignRepeat('weekly', friday, tuesday)).toBe('weekly')
    // An every-other-week rule's weeks move with it.
    expect(alignRepeat('FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,SU', new Date(2026, 9, 11), new Date(2026, 9, 12))).toBe(
      'FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,TU;WKST=TU',
    )
  })
})
