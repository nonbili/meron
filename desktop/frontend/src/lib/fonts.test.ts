import { describe, expect, it } from 'bun:test'
import {
  DEFAULT_FONT_SCALE,
  FONT_OPTIONS,
  MAX_FONT_SCALE,
  MAX_MESSAGE_FONT_SCALE,
  MIN_FONT_SCALE,
  clampFontScale,
  clampMessageFontScale,
  fontStack,
  isBuiltinFont,
  messageFrameZoom,
  messageFontStack,
  messageFrameFont,
  sanitizeFontChoice,
  sanitizeFontScale,
} from './fonts'

describe('fontStack', () => {
  it('leaves the default unset', () => {
    expect(fontStack('')).toBeNull()
    expect(fontStack('   ')).toBeNull()
  })

  it('resolves a built-in option to its stack', () => {
    const mono = FONT_OPTIONS.find((option) => option.id === 'mono')!
    expect(fontStack('mono')).toBe(mono.stack)
    expect(isBuiltinFont('mono')).toBe(true)
  })

  it('quotes a typed family name', () => {
    expect(fontStack('Fira Sans')).toBe("'Fira Sans'")
    expect(isBuiltinFont('Fira Sans')).toBe(false)
  })

  it('cannot break out of the quoted value', () => {
    expect(fontStack("Evil'; color: red; font-family: 'x")).toBe("'Evil color: red font-family: x'")
  })
})

describe('sanitizeFontChoice', () => {
  it('rejects non-strings so hydration keeps the current value', () => {
    expect(sanitizeFontChoice(42)).toBeNull()
    expect(sanitizeFontChoice(undefined)).toBeNull()
  })

  it('collapses whitespace and caps the length', () => {
    expect(sanitizeFontChoice('  Fira   Sans  ')).toBe('Fira Sans')
    expect(sanitizeFontChoice('a'.repeat(200))).toHaveLength(64)
  })
})

describe('font scale', () => {
  it('clamps to the supported range', () => {
    expect(clampFontScale(10)).toBe(MIN_FONT_SCALE)
    expect(clampFontScale(999)).toBe(MAX_FONT_SCALE)
    expect(clampFontScale(112.4)).toBe(112)
    expect(clampFontScale(Number.NaN)).toBe(DEFAULT_FONT_SCALE)
  })

  it('lets message bodies scale past the app-wide cap', () => {
    expect(clampMessageFontScale(999)).toBe(MAX_MESSAGE_FONT_SCALE)
    expect(clampMessageFontScale(380)).toBe(380)
    expect(clampFontScale(180)).toBe(MAX_FONT_SCALE)
    expect(sanitizeFontScale(380, MAX_MESSAGE_FONT_SCALE)).toBe(380)
  })

  it('only accepts stored numbers', () => {
    expect(sanitizeFontScale('120')).toBeNull()
    expect(sanitizeFontScale(120)).toBe(120)
  })
})

describe('message typography', () => {
  it('falls back to the interface font, then to the frame default', () => {
    expect(messageFontStack('', '')).toBeNull()
    expect(messageFontStack('', 'Fira Sans')).toStartWith("'Fira Sans', ")
    expect(messageFontStack('georgia', 'Fira Sans')).toStartWith("Georgia, 'Times New Roman', ")
  })

  it('multiplies the app size and the message size into one zoom', () => {
    expect(messageFrameZoom(100, 100)).toBe(1)
    expect(messageFrameZoom(150, 100)).toBe(1.5)
    expect(messageFrameZoom(100, 400)).toBe(4)
    expect(messageFrameZoom(150, 150)).toBe(2.25)
  })

  it('builds a frame font from the stored preferences', () => {
    const font = messageFrameFont({
      fontFamily: '',
      messageFontFamily: 'georgia',
      fontScale: 100,
      messageFontScale: 120,
    })
    expect(font.family).toStartWith('Georgia, ')
    expect(font.zoom).toBe(1.2)
  })
})
