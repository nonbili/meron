import { describe, expect, it } from 'bun:test'
import { isValidColor, luminance } from './color'
import {
  BUILTIN_THEMES,
  DEFAULT_DARK_ID,
  DEFAULT_LIGHT_ID,
  THEME_TOKEN_KEYS,
  TOKEN_CSS_VAR,
  cssVarStyle,
  deriveThemeTokens,
  isCustomThemeId,
  newCustomThemeId,
  sanitizeCustomThemes,
  type CustomThemeInput,
} from './themes'

const SAMPLE_INPUT: CustomThemeInput = {
  appearance: 'light',
  bgApp: '#f1f5f9',
  surface: '#ffffff',
  sidebar: '#0f172a',
  accent: '#4f46e5',
  text: '#0f172a',
}

describe('BUILTIN_THEMES', () => {
  it('includes both default ids with matching appearance', () => {
    const light = BUILTIN_THEMES.find((t) => t.id === DEFAULT_LIGHT_ID)
    const dark = BUILTIN_THEMES.find((t) => t.id === DEFAULT_DARK_ID)
    expect(light?.appearance).toBe('light')
    expect(dark?.appearance).toBe('dark')
  })

  it('every builtin fills every token slot with a non-empty string', () => {
    for (const theme of BUILTIN_THEMES) {
      for (const key of THEME_TOKEN_KEYS) {
        expect(typeof theme.tokens[key]).toBe('string')
        expect(theme.tokens[key].length).toBeGreaterThan(0)
      }
    }
  })

  it('has unique, non-custom ids and both appearances available', () => {
    const ids = BUILTIN_THEMES.map((t) => t.id)
    expect(BUILTIN_THEMES).toHaveLength(14)
    expect(new Set(ids).size).toBe(ids.length)
    for (const id of ids) expect(isCustomThemeId(id)).toBe(false)
    expect(BUILTIN_THEMES.filter((t) => t.appearance === 'light')).toHaveLength(7)
    expect(BUILTIN_THEMES.filter((t) => t.appearance === 'dark')).toHaveLength(7)
  })
})

describe('deriveThemeTokens', () => {
  it('produces valid colors for every color slot', () => {
    for (const appearance of ['light', 'dark'] as const) {
      const tokens = deriveThemeTokens({ ...SAMPLE_INPUT, appearance })
      for (const key of THEME_TOKEN_KEYS) {
        if (key === 'bubbleShadowIn' || key === 'bubbleShadowOut') continue
        expect(isValidColor(tokens[key])).toBe(true)
      }
    }
  })

  it('keeps text readable: secondary text contrasts with the app background', () => {
    const light = deriveThemeTokens(SAMPLE_INPUT)
    expect(luminance(light.textSecondary)).toBeLessThan(luminance(light.bgApp))
    const dark = deriveThemeTokens({
      appearance: 'dark',
      bgApp: '#090d16',
      surface: '#0f172a',
      sidebar: '#05070c',
      accent: '#6366f1',
      text: '#f8fafc',
    })
    expect(luminance(dark.textSecondary)).toBeGreaterThan(luminance(dark.bgApp))
  })

  it('passes the editor inputs through unchanged', () => {
    const tokens = deriveThemeTokens(SAMPLE_INPUT)
    expect(tokens.bgApp).toBe(SAMPLE_INPUT.bgApp)
    expect(tokens.bgChats).toBe(SAMPLE_INPUT.surface)
    expect(tokens.bgSidebar).toBe(SAMPLE_INPUT.sidebar)
    expect(tokens.accent).toBe(SAMPLE_INPUT.accent)
    expect(tokens.textPrimary).toBe(SAMPLE_INPUT.text)
  })
})

describe('cssVarStyle', () => {
  it('maps every slot to its --me-* var', () => {
    const style = cssVarStyle(BUILTIN_THEMES[0].tokens) as Record<string, string>
    for (const key of THEME_TOKEN_KEYS) {
      expect(style[TOKEN_CSS_VAR[key]]).toBe(BUILTIN_THEMES[0].tokens[key])
    }
  })
})

describe('sanitizeCustomThemes', () => {
  const valid = {
    id: newCustomThemeId(),
    name: 'My theme',
    appearance: 'light',
    source: SAMPLE_INPUT,
    tokens: deriveThemeTokens(SAMPLE_INPUT),
  }

  it('round-trips a valid theme', () => {
    const out = sanitizeCustomThemes([valid])
    expect(out).toHaveLength(1)
    expect(out![0]).toEqual(valid as never)
  })

  it('re-derives tokens when they are missing or corrupt', () => {
    const out = sanitizeCustomThemes([{ ...valid, tokens: { bgApp: 42 } }])
    expect(out).toHaveLength(1)
    expect(out![0].tokens).toEqual(deriveThemeTokens(SAMPLE_INPUT))
  })

  it('drops entries with bad ids, duplicate ids, or invalid sources', () => {
    const out = sanitizeCustomThemes([
      { ...valid, id: 'light' }, // not custom-prefixed
      valid,
      valid, // duplicate id
      { ...valid, id: newCustomThemeId(), source: { ...SAMPLE_INPUT, accent: 'tomato' } },
    ])
    expect(out).toHaveLength(1)
  })

  it('returns null for non-array input', () => {
    expect(sanitizeCustomThemes(undefined)).toBeNull()
    expect(sanitizeCustomThemes({})).toBeNull()
  })

  it('defaults a blank name', () => {
    const out = sanitizeCustomThemes([{ ...valid, name: '  ' }])
    expect(out![0].name).toBe('Custom theme')
  })
})
