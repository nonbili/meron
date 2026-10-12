import { describe, expect, it } from 'bun:test'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { isValidColor, luminance, parseColor, toOklch } from './color'
import {
  BUILTIN_THEMES,
  DEFAULT_DARK_ID,
  DEFAULT_LIGHT_ID,
  SURFACE_LADDER,
  THEME_TOKEN_KEYS,
  TOKEN_CSS_VAR,
  accentLabelColor,
  accentLabelVars,
  applySurfaceRules,
  builtinTheme,
  contrastRatio,
  cssVarStyle,
  deriveThemeTokens,
  isCustomThemeId,
  newCustomThemeId,
  parseThemeSource,
  sanitizeCustomThemes,
  serializeThemeSource,
  sideNavInkColor,
  whiteLabelAccent,
  withOpaqueSurface,
  type CustomThemeInput,
} from './themes'

const SAMPLE_INPUT: CustomThemeInput = {
  appearance: 'light',
  bgApp: '#f1f5f9',
  surface: '#ffffff',
  sideNav: '#0f172a',
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

  it('keeps CSS fallback colors in sync with the default theme palettes', () => {
    const css = readFileSync(new URL('../index.css', import.meta.url), 'utf8')
    for (const [id, selector] of [
      [DEFAULT_LIGHT_ID, ':root'],
      [DEFAULT_DARK_ID, '.dark'],
    ]) {
      const tokens = BUILTIN_THEMES.find((theme) => theme.id === id)!.tokens
      const declarations = Object.fromEntries(
        [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
          .filter((match) => match[1].trim().endsWith(selector))
          .flatMap((match) => [...match[2].matchAll(/(--me-[\w-]+):\s*([^;]+);/g)])
          .map((match) => [match[1], match[2].trim()]),
      )
      for (const key of THEME_TOKEN_KEYS) {
        expect(declarations[TOKEN_CSS_VAR[key]]).toBe(tokens[key])
      }
    }
  })

  it('steps every builtin down the surface ladder: list, then conversation, then rail', () => {
    const lightness = (color: string) => toOklch(color)!.l
    for (const theme of BUILTIN_THEMES) {
      const { bgChats, bgChat, bgSideNav } = theme.tokens
      const ladder = SURFACE_LADDER[theme.appearance]
      // Hex rounding moves lightness by a few thousandths.
      expect(lightness(bgChat) - lightness(bgChats)).toBeCloseTo(ladder.chat, 2)
      expect(lightness(bgSideNav) - lightness(bgChats)).toBeCloseTo(ladder.sideNav, 2)
    }
  })

  it('writes every builtin as the output of the surface rules', () => {
    for (const theme of BUILTIN_THEMES) {
      expect(applySurfaceRules(theme.tokens, theme.appearance)).toEqual(theme.tokens)
    }
  })

  it('keeps secondary text, the accent and bubble text readable', () => {
    for (const theme of BUILTIN_THEMES) {
      const { bgChats, bgChat, bubbleIn, textSecondary, accent } = theme.tokens
      expect(contrastRatio(textSecondary, bgChats)).toBeGreaterThanOrEqual(4.5)
      expect(contrastRatio(textSecondary, bgChat)).toBeGreaterThanOrEqual(4.5)
      expect(contrastRatio(accent, bgChats)).toBeGreaterThanOrEqual(4.5)
      expect(contrastRatio(accent, bubbleIn)).toBeGreaterThanOrEqual(4.5)
      expect(contrastRatio(theme.tokens.bubbleInText, bubbleIn)).toBeGreaterThanOrEqual(4.5)
      expect(contrastRatio(theme.tokens.bubbleOutText, theme.tokens.bubbleOut)).toBeGreaterThanOrEqual(4.5)
    }
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

// Tooltips and floating panels paint the surface (bg-chats) over other content,
// so it must be opaque in every theme; bgRaised is translucent in dark themes.
describe('surface opacity', () => {
  it('keeps the surface opaque in every built-in theme', () => {
    for (const theme of BUILTIN_THEMES) {
      expect({ id: theme.id, alpha: parseColor(theme.tokens.bgChats)?.a }).toEqual({ id: theme.id, alpha: 1 })
    }
  })

  it('keeps the surface opaque in derived light and dark themes', () => {
    for (const appearance of ['light', 'dark'] as const) {
      expect(parseColor(deriveThemeTokens({ ...SAMPLE_INPUT, appearance }).bgChats)?.a).toBe(1)
    }
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
      sideNav: '#05070c',
      accent: '#6366f1',
      text: '#f8fafc',
    })
    expect(luminance(dark.textSecondary)).toBeGreaterThan(luminance(dark.bgApp))
  })

  it('keeps bubble text readable after the bubbles move onto the ladder', () => {
    // A mid-grey dark surface lifts the outgoing bubble toward its light text.
    const tokens = deriveThemeTokens({
      appearance: 'dark',
      bgApp: '#444444',
      surface: '#555555',
      sideNav: '#333333',
      accent: '#0e7a58',
      text: '#f8fafc',
    })
    expect(contrastRatio(tokens.bubbleOutText, tokens.bubbleOut)).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio(tokens.bubbleInText, tokens.bubbleIn)).toBeGreaterThanOrEqual(4.5)
  })

  it('passes the editor inputs through unchanged', () => {
    const tokens = deriveThemeTokens(SAMPLE_INPUT)
    expect(tokens.bgApp).toBe(SAMPLE_INPUT.bgApp)
    expect(tokens.bgChats).toBe(SAMPLE_INPUT.surface)
    expect(tokens.bgSideNav).toBe(SAMPLE_INPUT.sideNav)
    expect(tokens.accent).toBe(SAMPLE_INPUT.accent)
    expect(tokens.textPrimary).toBe(SAMPLE_INPUT.text)
  })

  it('moves accents at either end of the scale until they read', () => {
    const white = deriveThemeTokens(parseThemeSource('light,#f0f2f1,#ffffff,#121a16,#ffffff,#1b211e')!)
    expect(white.accent).toBe('#767676')
    const black = deriveThemeTokens(parseThemeSource('dark,#000000,#000000,#000000,#000000,#ffffff')!)
    expect(contrastRatio(black.accent, black.bgChats)).toBeGreaterThanOrEqual(4.5)
  })
})

describe('cssVarStyle', () => {
  it('maps every slot to its --me-* var', () => {
    const style = cssVarStyle(BUILTIN_THEMES[0].tokens) as Record<string, string>
    for (const key of THEME_TOKEN_KEYS) {
      expect(style[TOKEN_CSS_VAR[key]]).toBe(BUILTIN_THEMES[0].tokens[key])
    }
  })
  it('scopes accent labels to the preview theme, including hover', () => {
    for (const theme of BUILTIN_THEMES) {
      expect(cssVarStyle(theme.tokens)).toMatchObject(accentLabelVars(theme.tokens))
    }
  })
})

describe('accent labels', () => {
  it('meets 4.5:1 contrast on every built-in normal and hover fill', () => {
    for (const theme of BUILTIN_THEMES) {
      for (const fill of [theme.tokens.accent, theme.tokens.accentHover]) {
        const channels = fill
          .slice(1)
          .match(/../g)!
          .map((hex) => {
            const value = parseInt(hex, 16) / 255
            return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
          })
        const luminance = channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
        const contrast = accentLabelColor(fill) === '#ffffff' ? 1.05 / (luminance + 0.05) : (luminance + 0.05) / 0.05
        expect(contrast).toBeGreaterThanOrEqual(4.5)
      }
    }
  })

  it('supports custom accents in hex, RGB, and HSL', () => {
    expect(accentLabelColor('#fff')).toBe('#000000')
    expect(accentLabelColor('rgb(0, 0, 0)')).toBe('#ffffff')
    expect(accentLabelColor('hsl(60, 100%, 50%)')).toBe('#000000')
  })

  it('prefers white wherever it meets 4.5:1, even when black contrasts more', () => {
    // Mist's teal: white 4.56:1, black a hair higher at 4.61:1.
    expect(accentLabelColor(builtinTheme('mist')!.tokens.accent)).toBe('#ffffff')
    expect(accentLabelColor('#767676')).toBe('#ffffff')
  })
})

describe('whiteLabelAccent', () => {
  it('gives white badge numbers 4.5:1 on every built-in accent', () => {
    for (const theme of BUILTIN_THEMES) {
      expect(contrastRatio('#ffffff', whiteLabelAccent(theme.tokens.accent))).toBeGreaterThanOrEqual(4.5)
    }
  })

  it('leaves an accent that already carries white text unchanged', () => {
    expect(whiteLabelAccent('#0e7a58')).toBe('#0e7a58')
  })
})

describe('sideNavInkColor', () => {
  const ink = (bgSideNav: string, textPrimary: string) =>
    sideNavInkColor({ ...BUILTIN_THEMES[0].tokens, bgSideNav, textPrimary })

  it('uses white on a dark rail', () => {
    expect(ink('#05070c', '#f8fafc')).toBe('#ffffff')
  })

  it('reuses the theme text on a light rail when it contrasts with the rail', () => {
    expect(ink('#f0f2f1', '#1b211e')).toBe('#1b211e')
  })

  it('falls back to black when the theme text is too close to the rail', () => {
    expect(ink('#777777', '#666666')).toBe('#000000')
  })
})

describe('serializeThemeSource / parseThemeSource', () => {
  it('writes appearance then the five colors as hex', () => {
    expect(serializeThemeSource(SAMPLE_INPUT)).toBe('light,#f1f5f9,#ffffff,#0f172a,#4f46e5,#0f172a')
    expect(serializeThemeSource({ ...SAMPLE_INPUT, accent: 'rgb(255, 0, 0)' })).toContain(',#ff0000,')
  })

  it('round-trips', () => {
    expect(parseThemeSource(serializeThemeSource(SAMPLE_INPUT))).toEqual(SAMPLE_INPUT)
  })

  it('tolerates whitespace, case and short hex', () => {
    expect(parseThemeSource('  Dark, #FFF ,#000,#111,#abc,#eee\n')).toEqual({
      appearance: 'dark',
      bgApp: '#ffffff',
      surface: '#000000',
      sideNav: '#111111',
      accent: '#aabbcc',
      text: '#eeeeee',
    })
  })

  it('rejects anything else', () => {
    expect(parseThemeSource('')).toBeNull()
    expect(parseThemeSource('light,#fff,#fff,#fff,#fff')).toBeNull()
    expect(parseThemeSource('light,#fff,#fff,#fff,#fff,#fff,#fff')).toBeNull()
    expect(parseThemeSource('dim,#fff,#fff,#fff,#fff,#fff')).toBeNull()
    expect(parseThemeSource('light,#fff,#fff,#fff,#fff,red')).toBeNull()
    expect(parseThemeSource('light,#fff,#fff,#fff,#fff,#ggg')).toBeNull()
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

  it('flattens a translucent surface over the background and re-derives the tokens', () => {
    const source = { ...SAMPLE_INPUT, appearance: 'dark', bgApp: '#000000', surface: 'rgba(255, 255, 255, 0.5)' }
    const out = sanitizeCustomThemes([{ ...valid, source, tokens: deriveThemeTokens(source as never) }])
    expect(out).toHaveLength(1)
    expect(out![0].source.surface).toBe('#808080')
    expect(out![0].tokens).toEqual(deriveThemeTokens({ ...source, surface: '#808080' } as never))
    expect(parseColor(out![0].tokens.bgChats)?.a).toBe(1)
  })

  it('leaves an opaque surface alone and flattens a translucent one', () => {
    expect(withOpaqueSurface(SAMPLE_INPUT)).toBe(SAMPLE_INPUT)
    const translucent = { ...SAMPLE_INPUT, bgApp: '#000000', surface: '#ffffff80' }
    expect(withOpaqueSurface(translucent)).toEqual({ ...translucent, surface: '#808080' })
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

describe('theme coverage', () => {
  // A literal color in a component class wins over the active theme: the
  // kanban and thread list headers painted Indigo Dark's #0f172a under every
  // dark theme. Surfaces must come from the token utilities (bg-header, ...).
  it('components take colors from theme tokens, not hard-coded hex values', () => {
    const offenders: string[] = []
    for (const file of new Bun.Glob('components/**/*.tsx').scanSync(fileURLToPath(new URL('..', import.meta.url)))) {
      if (file.endsWith('.test.tsx')) continue
      const source = readFileSync(new URL(`../${file}`, import.meta.url), 'utf8')
      for (const match of source.matchAll(/[\w:/-]*-\[#[0-9a-f]{3,8}\][\w/]*/gi)) {
        offenders.push(`${file}: ${match[0]}`)
      }
    }
    expect(offenders).toEqual([])
  })

  // Sortable rail items and rows are <div role="button" tabindex="0">. Without
  // this rule a mouse click leaves WebKit's blue focus ring on them, whatever
  // the theme; keyboard focus (:focus-visible) keeps its ring for reordering.
  it('hides the pointer-focus ring on div buttons but keeps it for keyboard focus', () => {
    const css = readFileSync(new URL('../index.css', import.meta.url), 'utf8')
    expect(css).toMatch(/\[role='button'\]:focus:not\(:focus-visible\)\s*\{[^}]*outline:\s*none;/)
  })
})
