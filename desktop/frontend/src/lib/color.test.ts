import { describe, expect, it } from 'bun:test'
import { darken, formatColor, isValidColor, lighten, luminance, mix, parseColor, withAlpha } from './color'

describe('parseColor', () => {
  it('parses the syntaxes email writes: 4-digit hex, hsl, space-separated rgb', () => {
    expect(parseColor('#0009')).toEqual({ r: 0, g: 0, b: 0, a: 0x99 / 255 })
    expect(parseColor('hsl(0, 0%, 0%)')).toEqual({ r: 0, g: 0, b: 0, a: 1 })
    expect(parseColor('hsl(210 50% 90%)')).toEqual({ r: 217, g: 230, b: 242, a: 1 })
    expect(parseColor('rgb(0 0 0 / 50%)')).toEqual({ r: 0, g: 0, b: 0, a: 0.5 })
    expect(parseColor('rgb(50%, 0, 0)')).toEqual({ r: 128, g: 0, b: 0, a: 1 })
  })

  it('accepts a hue in any CSS angle unit', () => {
    const teal = { r: 64, g: 191, b: 191, a: 1 }
    for (const angle of ['0.5turn', '3.14rad', '200grad', '180deg', '180']) {
      expect(parseColor(`hsl(${angle} 50% 50%)`)).toEqual(teal)
    }
  })

  it('reads units and function names case-insensitively, as CSS does', () => {
    expect(parseColor('HSL(180DEG 50% 50%)')).toEqual({ r: 64, g: 191, b: 191, a: 1 })
    expect(parseColor('RGB(0, 0, 0)')).toEqual({ r: 0, g: 0, b: 0, a: 1 })
  })

  it('holds hsl to its own channel grammar', () => {
    // A hue is an angle and the other two are percentages; neither swaps.
    for (const invalid of ['hsl(50% 50% 50%)', 'hsl(0 1 1)', 'hsl(0, 50, 50)']) {
      expect(parseColor(invalid)).toBeNull()
      expect(isValidColor(invalid)).toBe(false)
    }
    expect(parseColor('hsl(0, 0%, 0%)')).toEqual({ r: 0, g: 0, b: 0, a: 1 })
  })

  it('rejects a mix of the comma and space grammars', () => {
    for (const invalid of ['rgb(1/2/3)', 'rgb(1, 2 3)', 'hsl(0, 50% 50%)', 'rgb(0 0 0 / 1 / 1)']) {
      expect(parseColor(invalid)).toBeNull()
      expect(isValidColor(invalid)).toBe(false)
    }
  })

  it('rejects components that only start with a number', () => {
    // `parseFloat` would read each of these as a valid channel.
    for (const invalid of ['rgb(12px, 0, 0)', 'rgb(10junk, 20, 30)', 'hsl(0x, 50y, 50z)', 'rgb(0, 0)']) {
      expect(parseColor(invalid)).toBeNull()
      expect(isValidColor(invalid)).toBe(false)
    }
  })

  it('parses 6-digit hex', () => {
    expect(parseColor('#4f46e5')).toEqual({ r: 0x4f, g: 0x46, b: 0xe5, a: 1 })
  })

  it('parses 3-digit hex', () => {
    expect(parseColor('#fff')).toEqual({ r: 255, g: 255, b: 255, a: 1 })
  })

  it('parses 8-digit hex with alpha', () => {
    const parsed = parseColor('#ff000080')
    expect(parsed?.r).toBe(255)
    expect(parsed?.a).toBeCloseTo(0.5, 1)
  })

  it('parses rgb() and rgba()', () => {
    expect(parseColor('rgb(15, 23, 42)')).toEqual({ r: 15, g: 23, b: 42, a: 1 })
    expect(parseColor('rgba(248, 250, 252, 0.94)')).toEqual({ r: 248, g: 250, b: 252, a: 0.94 })
  })

  it('rejects garbage', () => {
    expect(parseColor('')).toBeNull()
    expect(parseColor('red')).toBeNull()
    expect(parseColor('#12345')).toBeNull()
    expect(parseColor('rgb(1,2)')).toBeNull()
  })
})

describe('formatColor', () => {
  it('round-trips opaque hex', () => {
    expect(formatColor(parseColor('#4f46e5')!)).toBe('#4f46e5')
  })

  it('emits rgba for translucent colors', () => {
    expect(formatColor({ r: 30, g: 41, b: 59, a: 0.6 })).toBe('rgba(30, 41, 59, 0.6)')
  })
})

describe('mix / lighten / darken / withAlpha', () => {
  it('mix at 0 and 1 returns the endpoints', () => {
    expect(mix('#000000', '#ffffff', 0)).toBe('#000000')
    expect(mix('#000000', '#ffffff', 1)).toBe('#ffffff')
  })

  it('mix at 0.5 is the midpoint', () => {
    expect(mix('#000000', '#ffffff', 0.5)).toBe('#808080')
  })

  it('lighten moves toward white, darken toward black', () => {
    expect(luminance(lighten('#4f46e5', 0.2))).toBeGreaterThan(luminance('#4f46e5'))
    expect(luminance(darken('#4f46e5', 0.2))).toBeLessThan(luminance('#4f46e5'))
  })

  it('withAlpha keeps the channel values', () => {
    expect(withAlpha('#1e293b', 0.6)).toBe('rgba(30, 41, 59, 0.6)')
  })

  it('returns input unchanged for unparseable colors', () => {
    expect(mix('bogus', '#fff', 0.5)).toBe('bogus')
    expect(withAlpha('bogus', 0.5)).toBe('bogus')
  })
})

describe('isValidColor', () => {
  it('accepts hex and rgba, rejects names', () => {
    expect(isValidColor('#abc')).toBe(true)
    expect(isValidColor('rgba(0,0,0,0.5)')).toBe(true)
    expect(isValidColor('tomato')).toBe(false)
  })
})
