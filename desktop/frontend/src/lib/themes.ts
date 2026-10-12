import type { CSSProperties } from 'react'
import {
  darken,
  flatten,
  fromOklch,
  isValidColor,
  lighten,
  mix,
  parseColor,
  toHex,
  toOklch,
  withAlpha,
  withLightness,
} from './color'

// Theme registry and derivation. A theme is a complete set of values for the
// `--me-*` CSS custom properties in index.css. Built-ins live here; custom
// themes are derived from a handful of editor inputs (CustomThemeInput) and
// persisted with their full token map in settings (`custom_themes`).

export type Appearance = 'light' | 'dark'

/**
 * One value per CSS custom property slot. Colors are CSS color strings;
 * the two shadow slots are full box-shadow values.
 */
export type ThemeTokens = {
  bgApp: string
  bgChat: string
  bgChatOverlay: string
  bgSideNav: string
  bgChats: string
  bgHeader: string
  bgHover: string
  bgRaised: string
  bgActive: string
  border: string
  textPrimary: string
  textSecondary: string
  accent: string
  accentHover: string
  bubbleIn: string
  bubbleInText: string
  bubbleOut: string
  bubbleOutText: string
  composerBg: string
  composerBorder: string
  bubbleShadowIn: string
  bubbleShadowOut: string
}

export const TOKEN_CSS_VAR: Record<keyof ThemeTokens, string> = {
  bgApp: '--me-bg-app',
  bgChat: '--me-bg-chat',
  bgChatOverlay: '--me-bg-chat-overlay',
  bgSideNav: '--me-bg-sidenav',
  bgChats: '--me-bg-chats',
  bgHeader: '--me-bg-header',
  bgHover: '--me-bg-hover',
  bgRaised: '--me-bg-raised',
  bgActive: '--me-bg-active',
  border: '--me-border',
  textPrimary: '--me-text-primary',
  textSecondary: '--me-text-secondary',
  accent: '--me-accent',
  accentHover: '--me-accent-hover',
  bubbleIn: '--me-bubble-in',
  bubbleInText: '--me-bubble-in-text',
  bubbleOut: '--me-bubble-out',
  bubbleOutText: '--me-bubble-out-text',
  composerBg: '--me-composer-bg',
  composerBorder: '--me-composer-border',
  bubbleShadowIn: '--me-bubble-shadow-in',
  bubbleShadowOut: '--me-bubble-shadow-out',
}

export const THEME_TOKEN_KEYS = Object.keys(TOKEN_CSS_VAR) as (keyof ThemeTokens)[]

export type ThemeDef = {
  /** Built-ins: "light", "dark", "mist", ... Custom themes: "custom-<random>". */
  id: string
  name: string
  appearance: Appearance
  tokens: ThemeTokens
}

/** The 6 inputs the custom theme editor exposes; everything else is derived. */
export type CustomThemeInput = {
  appearance: Appearance
  /** Window background behind everything. */
  bgApp: string
  /** Panels: thread list, header, composer, incoming bubbles. */
  surface: string
  /** The account rail / side navigation. */
  sideNav: string
  accent: string
  /** Primary text color. */
  text: string
}

export type CustomTheme = ThemeDef & { source: CustomThemeInput }

/**
 * Where every theme places its surfaces, as OKLCH lightness offsets: from the
 * list surface (bgChats) for the panes and row states, from the conversation
 * background (bgChat) for the bubbles. Each token keeps its own hue and
 * chroma; only its lightness is placed. Panes are told apart by these steps
 * rather than divider lines, so every theme reads the same way: rail darkest,
 * conversation in between, list lightest (a dark theme's bubbles and row
 * states rise above them). Light steps are smaller because the eye separates
 * light tones more easily.
 */
export const SURFACE_LADDER = {
  light: {
    chat: -0.035,
    sideNav: -0.07,
    hover: -0.045,
    active: -0.08,
    border: -0.09,
    bubbleIn: 0.035,
    bubbleOut: -0.035,
  },
  dark: { chat: -0.04, sideNav: -0.08, hover: 0.05, active: 0.08, border: 0.1, bubbleIn: 0.09, bubbleOut: 0.16 },
} satisfies Record<Appearance, Record<string, number>>

/** WCAG AA for normal text: secondary text and the accent (sender names, links, active tabs). */
const TEXT_MIN_CONTRAST = 4.5

/** `color` moved in lightness, away from `backgrounds`, until it reaches `min` contrast on all of them. */
function withMinContrast(color: string, backgrounds: string[], min: number, direction: 1 | -1): string {
  const oklch = toOklch(color)
  if (!oklch) return color
  let out = toHex(color) ?? color
  // Stop only at the end of the scale being moved toward: white sits at the
  // top of it and can still darken, black at the bottom and can still lighten.
  const hasRoom = () => (direction < 0 ? oklch.l > 0 : oklch.l < 1)
  while (Math.min(...backgrounds.map((bg) => contrastRatio(out, bg))) < min && hasRoom()) {
    oklch.l = Math.min(1, Math.max(0, oklch.l + direction * 0.005))
    out = fromOklch(oklch)
  }
  return out
}

/** `ink` at the alpha that lands on lightness `target` when composited over `base`. */
function tintToLightness(ink: string, base: string, target: number): string {
  let low = 0
  let high = 1
  for (let step = 0; step < 20; step++) {
    const alpha = (low + high) / 2
    const lightness = toOklch(flatten(withAlpha(ink, alpha), base))?.l ?? target
    const baseLightness = toOklch(base)?.l ?? target
    // Past the target in the ink's direction: back off.
    if (Math.abs(lightness - baseLightness) > Math.abs(target - baseLightness)) high = alpha
    else low = alpha
  }
  return withAlpha(ink, Math.round(((low + high) / 2) * 1000) / 1000)
}

/**
 * `text` made readable on `bg`: moved further to the side of `bg` it already
 * sits on (lighter or darker), or across to the other side when that side
 * runs out of room before reaching `min`.
 */
function readableOn(text: string, bg: string, min: number): string {
  const side = (toOklch(text)?.l ?? 0) > (toOklch(bg)?.l ?? 0) ? 1 : -1
  const kept = withMinContrast(text, [bg], min, side)
  return contrastRatio(kept, bg) >= min ? kept : withMinContrast(text, [bg], min, side === 1 ? -1 : 1)
}

/**
 * Place a theme's surfaces on SURFACE_LADDER and bring its secondary text,
 * accent and bubble text up to TEXT_MIN_CONTRAST. Custom themes are derived through this, and
 * every built-in is written as its output (themes.test.ts holds them to it).
 * The side nav is left alone: it is a custom-theme editor input.
 */
export function applySurfaceRules(tokens: ThemeTokens, appearance: Appearance): ThemeTokens {
  const light = appearance === 'light'
  const ladder = SURFACE_LADDER[appearance]
  const list = toOklch(tokens.bgChats)?.l ?? 0
  const bgChat = withLightness(tokens.bgChat, list + ladder.chat)
  const chat = list + ladder.chat
  const border = withLightness(flatten(tokens.border, tokens.bgChats), list + ladder.border)
  const bubbleIn = withLightness(tokens.bubbleIn, chat + ladder.bubbleIn)
  // Text darkens on a light theme and lightens on a dark one to gain contrast.
  const away = light ? -1 : 1
  const accent = withMinContrast(tokens.accent, [tokens.bgChats, bubbleIn], TEXT_MIN_CONTRAST, away)
  const accentL = toOklch(accent)?.l ?? 0
  const bubbleOut = withLightness(tokens.bubbleOut, chat + ladder.bubbleOut)
  return {
    ...tokens,
    bgChat,
    bgChatOverlay: withAlpha(bgChat, light ? 0.94 : 0.95),
    // Translucent, so a hover reads the same on the list, the conversation
    // background and the bubbles alike; tuned to the ladder over the list.
    bgHover: tintToLightness(tokens.textPrimary, tokens.bgChats, list + ladder.hover),
    bgActive: withLightness(flatten(tokens.bgActive, tokens.bgChats), list + ladder.active),
    border,
    composerBorder: border,
    textSecondary: withMinContrast(tokens.textSecondary, [tokens.bgChats, bgChat], TEXT_MIN_CONTRAST, away),
    accent,
    accentHover: withLightness(accent, accentL + (light ? -0.05 : 0.04)),
    bubbleIn,
    // The bubbles moved, so their text follows to stay readable on them.
    bubbleInText: readableOn(tokens.bubbleInText, bubbleIn, TEXT_MIN_CONTRAST),
    bubbleOut,
    bubbleOutText: readableOn(tokens.bubbleOutText, bubbleOut, TEXT_MIN_CONTRAST),
  }
}

/**
 * Expand the 6 editor inputs into a full token map. Formulas are tuned so the
 * Meron Light/Dark inputs reproduce (closely) the hand-picked index.css values.
 */
export function deriveThemeTokens(input: CustomThemeInput): ThemeTokens {
  const { appearance, bgApp, surface, sideNav, accent, text } = input
  const light = appearance === 'light'

  const bgChat = bgApp
  const border = mix(surface, text, 0.12)
  return applySurfaceRules(
    {
      bgApp,
      bgChat,
      bgChatOverlay: withAlpha(bgChat, light ? 0.94 : 0.95),
      bgSideNav: sideNav,
      bgChats: surface,
      bgHeader: surface,
      bgHover: light ? mix(surface, text, 0.06) : withAlpha(lighten(surface, 0.08), 0.6),
      bgRaised: light ? mix(surface, bgApp, 0.5) : withAlpha(surface, 0.4),
      bgActive: light ? mix(surface, text, 0.12) : lighten(surface, 0.1),
      border,
      textPrimary: text,
      textSecondary: mix(text, bgApp, 0.45),
      accent,
      accentHover: light ? darken(accent, 0.1) : lighten(accent, 0.12),
      bubbleIn: light ? surface : lighten(surface, 0.08),
      bubbleInText: text,
      bubbleOut: light ? mix(accent, '#ffffff', 0.8) : mix(accent, surface, 0.55),
      bubbleOutText: light ? darken(accent, 0.45) : mix(accent, '#ffffff', 0.75),
      composerBg: surface,
      composerBorder: border,
      bubbleShadowIn: light
        ? `0 2px 8px -2px ${withAlpha(text, 0.08)}, 0 1px 3px -1px ${withAlpha(text, 0.04)}`
        : '0 4px 12px -3px rgba(0, 0, 0, 0.3), 0 1px 4px -2px rgba(0, 0, 0, 0.2)',
      bubbleShadowOut: light
        ? `0 2px 8px -2px ${withAlpha(accent, 0.12)}, 0 1px 3px -1px ${withAlpha(accent, 0.06)}`
        : '0 4px 12px -3px rgba(0, 0, 0, 0.4), 0 1px 4px -2px rgba(0, 0, 0, 0.3)',
    },
    appearance,
  )
}

// "Meron Light" / "Meron Dark" lean melon green to match the app icon, over
// neutrals with a faint green cast.
// MERON_LIGHT and MERON_DARK mirror the `:root` / `.dark` fallbacks in index.css;
// keep both in sync.
const MERON_LIGHT: ThemeTokens = {
  bgApp: '#f0f2f1',
  bgChat: '#f2f4f3',
  bgChatOverlay: 'rgba(242, 244, 243, 0.94)',
  bgSideNav: '#e6e9e7',
  bgChats: '#ffffff',
  bgHeader: '#ffffff',
  bgHover: 'rgba(27, 33, 30, 0.07)',
  bgRaised: '#f7f9f8',
  bgActive: '#e1e6e3',
  border: '#dde3df',
  textPrimary: '#1b211e',
  textSecondary: '#65716b',
  accent: '#0e7a58',
  accentHover: '#006b4a',
  bubbleIn: '#ffffff',
  bubbleInText: '#1b211e',
  bubbleOut: '#dcede5',
  bubbleOutText: '#14543e',
  composerBg: '#ffffff',
  composerBorder: '#dde3df',
  bubbleShadowIn: '0 2px 8px -2px rgba(27, 33, 30, 0.08), 0 1px 3px -1px rgba(27, 33, 30, 0.04)',
  bubbleShadowOut: '0 2px 8px -2px rgba(14, 122, 88, 0.12), 0 1px 3px -1px rgba(14, 122, 88, 0.06)',
}

const MERON_DARK: ThemeTokens = {
  bgApp: '#0c100e',
  bgChat: '#0d1210',
  bgChatOverlay: 'rgba(13, 18, 16, 0.95)',
  bgSideNav: '#060908',
  bgChats: '#151b18',
  bgHeader: '#151b18',
  bgHover: 'rgba(242, 245, 243, 0.06)',
  bgRaised: 'rgba(21, 27, 24, 0.4)',
  bgActive: '#262f2a',
  border: '#29352f',
  textPrimary: '#f2f5f3',
  textSecondary: '#98a39d',
  accent: '#40a984',
  accentHover: '#4eb690',
  bubbleIn: '#1f2823',
  bubbleInText: '#f2f5f3',
  bubbleOut: '#153f33',
  bubbleOutText: '#d6eee2',
  composerBg: '#151b18',
  composerBorder: '#29352f',
  bubbleShadowIn: '0 4px 12px -3px rgba(0, 0, 0, 0.3), 0 1px 4px -2px rgba(0, 0, 0, 0.2)',
  bubbleShadowOut: '0 4px 12px -3px rgba(0, 0, 0, 0.4), 0 1px 4px -2px rgba(0, 0, 0, 0.3)',
}

const INDIGO_LIGHT: ThemeTokens = {
  bgApp: '#f1f5f9',
  bgChat: '#f0f4f8',
  bgChatOverlay: 'rgba(240, 244, 248, 0.94)',
  bgSideNav: '#e3e9ee',
  bgChats: '#ffffff',
  bgHeader: '#ffffff',
  bgHover: 'rgba(15, 23, 42, 0.07)',
  bgRaised: '#f8fafc',
  bgActive: '#dfe5ed',
  border: '#dce2ea',
  textPrimary: '#0f172a',
  textSecondary: '#607086',
  accent: '#6558cc',
  accentHover: '#5848bb',
  bubbleIn: '#ffffff',
  bubbleInText: '#0f172a',
  bubbleOut: '#e0e7ff',
  bubbleOutText: '#312e81',
  composerBg: '#ffffff',
  composerBorder: '#dce2ea',
  bubbleShadowIn: '0 2px 8px -2px rgba(15, 23, 42, 0.08), 0 1px 3px -1px rgba(15, 23, 42, 0.04)',
  bubbleShadowOut: '0 2px 8px -2px rgba(101, 88, 204, 0.12), 0 1px 3px -1px rgba(101, 88, 204, 0.06)',
}

const INDIGO_DARK: ThemeTokens = {
  bgApp: '#090d16',
  bgChat: '#0b0f18',
  bgChatOverlay: 'rgba(11, 15, 24, 0.95)',
  bgSideNav: '#05070c',
  bgChats: '#0f172a',
  bgHeader: '#0f172a',
  bgHover: 'rgba(248, 250, 252, 0.06)',
  bgRaised: 'rgba(15, 23, 42, 0.4)',
  bgActive: '#202b3d',
  border: '#253042',
  textPrimary: '#f8fafc',
  textSecondary: '#94a3b8',
  accent: '#897fe0',
  accentHover: '#958bee',
  bubbleIn: '#192435',
  bubbleInText: '#f8fafc',
  bubbleOut: '#2a2577',
  bubbleOutText: '#e0e7ff',
  composerBg: '#0f172a',
  composerBorder: '#253042',
  bubbleShadowIn: '0 4px 12px -3px rgba(0, 0, 0, 0.3), 0 1px 4px -2px rgba(0, 0, 0, 0.2)',
  bubbleShadowOut: '0 4px 12px -3px rgba(0, 0, 0, 0.4), 0 1px 4px -2px rgba(0, 0, 0, 0.3)',
}

// The remaining built-ins start from the editor derivation, then pin the roles
// that need hand-tuning to feel coherent across the full app surface.
const MIST: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'light',
    bgApp: '#edf4f7',
    surface: '#ffffff',
    sideNav: '#e0eaee',
    accent: '#2996a6',
    text: '#14323c',
  }),
  bgChat: '#eef5f8',
  bgChatOverlay: 'rgba(238, 245, 248, 0.94)',
  bgHover: 'rgba(20, 50, 60, 0.08)',
  bgRaised: '#f4fafb',
  bgActive: '#d6e9ee',
  border: '#d4e5ea',
  textSecondary: '#5b727b',
  bubbleIn: '#ffffff',
  bubbleOut: '#d3eef2',
  bubbleOutText: '#0e5663',
  composerBg: '#ffffff',
  composerBorder: '#d4e5ea',
  bubbleShadowIn: '0 2px 8px -2px rgba(20, 50, 60, 0.1), 0 1px 3px -1px rgba(20, 50, 60, 0.06)',
  bubbleShadowOut: '0 2px 8px -2px rgba(41, 150, 166, 0.18), 0 1px 3px -1px rgba(41, 150, 166, 0.1)',
}

const PAPER: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'light',
    bgApp: '#f4f1ea',
    surface: '#fffdf8',
    sideNav: '#eae6dc',
    accent: '#64748b',
    text: '#2f3a3d',
  }),
  bgChat: '#f4f1ea',
  bgChatOverlay: 'rgba(244, 241, 234, 0.94)',
  bgHover: 'rgba(47, 58, 61, 0.08)',
  bgRaised: '#faf6ee',
  bgActive: '#eae2d5',
  border: '#e8dece',
  textSecondary: '#6a6f6c',
  bubbleIn: '#fffdf8',
  bubbleOut: '#dfe7ec',
  bubbleOutText: '#334155',
  composerBg: '#fffdf8',
  composerBorder: '#e8dece',
  bubbleShadowIn: '0 2px 8px -2px rgba(47, 58, 61, 0.1), 0 1px 3px -1px rgba(47, 58, 61, 0.06)',
  bubbleShadowOut: '0 2px 8px -2px rgba(100, 116, 139, 0.16), 0 1px 3px -1px rgba(100, 116, 139, 0.1)',
}

const DAWN: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'light',
    bgApp: '#f7ede8',
    surface: '#fffaf7',
    sideNav: '#efe1da',
    accent: '#c06c84',
    text: '#4a3f4d',
  }),
  bgChat: '#f7ede8',
  bgChatOverlay: 'rgba(247, 237, 232, 0.94)',
  bgHover: 'rgba(74, 63, 77, 0.09)',
  bgRaised: '#fff6f2',
  bgActive: '#eedcd5',
  border: '#ebd9d1',
  textSecondary: '#766970',
  bubbleIn: '#fffaf7',
  bubbleOut: '#f8dcdc',
  bubbleOutText: '#753849',
  composerBg: '#fffaf7',
  composerBorder: '#ebd9d1',
  bubbleShadowIn: '0 2px 8px -2px rgba(74, 63, 77, 0.1), 0 1px 3px -1px rgba(74, 63, 77, 0.06)',
  bubbleShadowOut: '0 2px 8px -2px rgba(192, 108, 132, 0.18), 0 1px 3px -1px rgba(192, 108, 132, 0.1)',
}

const GRAPHITE: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'dark',
    bgApp: '#181a1f',
    surface: '#23262d',
    sideNav: '#111318',
    accent: '#8b9bb4',
    text: '#eef0f3',
  }),
  bgChat: '#1a1c21',
  bgChatOverlay: 'rgba(26, 28, 33, 0.95)',
  bgHover: 'rgba(238, 240, 243, 0.07)',
  bgRaised: '#202329',
  bgActive: '#363a44',
  border: '#3a3f4a',
  textSecondary: '#a8b0bc',
  bubbleIn: '#2e323c',
  bubbleOut: '#3c4552',
  bubbleOutText: '#eef3f8',
  composerBg: '#23262d',
  composerBorder: '#3a3f4a',
}

const MIDNIGHT: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'dark',
    bgApp: '#0b1120',
    surface: '#111827',
    sideNav: '#040712',
    accent: '#55add5',
    text: '#f8fafc',
  }),
  bgChat: '#070f1f',
  bgChatOverlay: 'rgba(7, 15, 31, 0.95)',
  bgHover: 'rgba(248, 250, 252, 0.06)',
  bgRaised: '#101827',
  bgActive: '#212c3e',
  border: '#223148',
  textSecondary: '#94a3b8',
  bubbleIn: '#192436',
  bubbleOut: '#193851',
  bubbleOutText: '#dff6ff',
  composerBg: '#111827',
  composerBorder: '#223148',
}

const FOREST: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'dark',
    bgApp: '#101813',
    surface: '#17231c',
    sideNav: '#09100c',
    accent: '#7ccf9b',
    text: '#f0f6ef',
  }),
  bgChat: '#101914',
  bgChatOverlay: 'rgba(16, 25, 20, 0.95)',
  bgHover: 'rgba(240, 246, 239, 0.06)',
  bgRaised: '#152018',
  bgActive: '#25392d',
  border: '#283f31',
  textSecondary: '#a6b8aa',
  bubbleIn: '#1f3025',
  bubbleOut: '#234633',
  bubbleOutText: '#e2f8e9',
  composerBg: '#17231c',
  composerBorder: '#283f31',
}

const HONEY: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'light',
    bgApp: '#f7f1e6',
    surface: '#fffdf7',
    sideNav: '#ede5d6',
    accent: '#b07c10',
    text: '#3a3122',
  }),
  bgChat: '#f7f1e6',
  bgChatOverlay: 'rgba(247, 241, 230, 0.94)',
  bgHover: 'rgba(58, 49, 34, 0.08)',
  bgRaised: '#faf4e8',
  bgActive: '#ede2c9',
  border: '#e9dec4',
  textSecondary: '#786d56',
  bubbleIn: '#fffdf7',
  bubbleOut: '#f4e5c3',
  bubbleOutText: '#6e4d09',
  composerBg: '#fffdf7',
  composerBorder: '#e9dec4',
  bubbleShadowIn: '0 2px 8px -2px rgba(58, 49, 34, 0.1), 0 1px 3px -1px rgba(58, 49, 34, 0.06)',
  bubbleShadowOut: '0 2px 8px -2px rgba(176, 124, 16, 0.16), 0 1px 3px -1px rgba(176, 124, 16, 0.1)',
}

const LILAC: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'light',
    bgApp: '#f2f0f8',
    surface: '#fdfcff',
    sideNav: '#e6e3ef',
    accent: '#7a5bc4',
    text: '#34304a',
  }),
  bgChat: '#f2f0f8',
  bgChatOverlay: 'rgba(242, 240, 248, 0.94)',
  bgHover: 'rgba(52, 48, 74, 0.08)',
  bgRaised: '#f6f4fb',
  bgActive: '#e4def3',
  border: '#e1dbed',
  textSecondary: '#6f6985',
  bubbleIn: '#fdfcff',
  bubbleOut: '#eae0fa',
  bubbleOutText: '#4b3389',
  composerBg: '#fdfcff',
  composerBorder: '#e1dbed',
  bubbleShadowIn: '0 2px 8px -2px rgba(52, 48, 74, 0.1), 0 1px 3px -1px rgba(52, 48, 74, 0.06)',
  bubbleShadowOut: '0 2px 8px -2px rgba(122, 91, 196, 0.16), 0 1px 3px -1px rgba(122, 91, 196, 0.1)',
}

const PLUM: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'dark',
    bgApp: '#151019',
    surface: '#1f1826',
    sideNav: '#0b080f',
    accent: '#b48ae0',
    text: '#f2eef6',
  }),
  bgChat: '#151017',
  bgChatOverlay: 'rgba(21, 16, 23, 0.95)',
  bgHover: 'rgba(242, 238, 246, 0.06)',
  bgRaised: '#1c1522',
  bgActive: '#332945',
  border: '#392e49',
  textSecondary: '#a89db8',
  bubbleIn: '#2c2337',
  bubbleOut: '#3f3059',
  bubbleOutText: '#ecdffb',
  composerBg: '#1f1826',
  composerBorder: '#392e49',
}

const EMBER: ThemeTokens = {
  ...deriveThemeTokens({
    appearance: 'dark',
    bgApp: '#181210',
    surface: '#231a15',
    sideNav: '#0e0a08',
    accent: '#e1854c',
    text: '#f6efe9',
  }),
  bgChat: '#18120f',
  bgChatOverlay: 'rgba(24, 18, 15, 0.95)',
  bgHover: 'rgba(246, 239, 233, 0.06)',
  bgRaised: '#201813',
  bgActive: '#3b2c21',
  border: '#413126',
  textSecondary: '#b4a294',
  bubbleIn: '#31251e',
  bubbleOut: '#4e321f',
  bubbleOutText: '#fae3cf',
  composerBg: '#231a15',
  composerBorder: '#413126',
}

export const BUILTIN_THEMES: ThemeDef[] = [
  { id: 'light', name: 'Meron Light', appearance: 'light', tokens: MERON_LIGHT },
  { id: 'indigo', name: 'Indigo', appearance: 'light', tokens: INDIGO_LIGHT },
  { id: 'dark', name: 'Meron Dark', appearance: 'dark', tokens: MERON_DARK },
  { id: 'indigo-dark', name: 'Indigo Dark', appearance: 'dark', tokens: INDIGO_DARK },
  { id: 'mist', name: 'Mist', appearance: 'light', tokens: MIST },
  { id: 'paper', name: 'Paper', appearance: 'light', tokens: PAPER },
  { id: 'dawn', name: 'Dawn', appearance: 'light', tokens: DAWN },
  { id: 'honey', name: 'Honey', appearance: 'light', tokens: HONEY },
  { id: 'lilac', name: 'Lilac', appearance: 'light', tokens: LILAC },
  { id: 'graphite', name: 'Graphite', appearance: 'dark', tokens: GRAPHITE },
  { id: 'midnight', name: 'Midnight', appearance: 'dark', tokens: MIDNIGHT },
  { id: 'forest', name: 'Forest', appearance: 'dark', tokens: FOREST },
  { id: 'plum', name: 'Plum', appearance: 'dark', tokens: PLUM },
  { id: 'ember', name: 'Ember', appearance: 'dark', tokens: EMBER },
]

export const DEFAULT_LIGHT_ID = 'light'
export const DEFAULT_DARK_ID = 'dark'

export function builtinTheme(id: string): ThemeDef | undefined {
  return BUILTIN_THEMES.find((theme) => theme.id === id)
}

export function defaultThemeId(appearance: Appearance): string {
  return appearance === 'light' ? DEFAULT_LIGHT_ID : DEFAULT_DARK_ID
}

/** Editor seed for a new custom theme: the default theme's source palette. */
export function defaultCustomInput(appearance: Appearance): CustomThemeInput {
  return appearance === 'light'
    ? { appearance, bgApp: '#f0f2f1', surface: '#ffffff', sideNav: '#e6e9e7', accent: '#0e7a58', text: '#1b211e' }
    : { appearance, bgApp: '#0c100e', surface: '#151b18', sideNav: '#060908', accent: '#40a984', text: '#f2f5f3' }
}

export function newCustomThemeId(): string {
  return `custom-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`
}

export function isCustomThemeId(id: string): boolean {
  return id.startsWith('custom-')
}

/**
 * Inline-style record assigning every token to its CSS var. Used to scope a
 * theme to a subtree (swatches, editor preview): inline vars on a wrapper
 * override the inherited ones, so the token utilities (`bg-app`, `bg-accent`, ...) work inside it.
 */
export function cssVarStyle(tokens: ThemeTokens): CSSProperties {
  const style: Record<string, string> = accentLabelVars(tokens)
  for (const key of THEME_TOKEN_KEYS) {
    style[TOKEN_CSS_VAR[key]] = tokens[key]
  }
  return style as CSSProperties
}

/** WCAG relative luminance of an opaque color; null if unparseable. */
function relativeLuminance(color: string): number | null {
  const rgb = parseColor(color)
  if (!rgb) return null
  const linear = [rgb.r, rgb.g, rgb.b].map((channel) => {
    const value = channel / 255
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  })
  return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
}

/** WCAG contrast ratio between two opaque colors, 1..21; 1 if either is unparseable. */
export function contrastRatio(a: string, b: string): number {
  const la = relativeLuminance(a)
  const lb = relativeLuminance(b)
  if (la === null || lb === null) return 1
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05)
}

/**
 * Label for an opaque fill: white whenever it meets WCAG AA there, as labels
 * and icons on a colored button usually are, else whichever of white and
 * black contrasts more.
 */
export function accentLabelColor(color: string): string {
  const luminance = relativeLuminance(color)
  if (luminance === null) return '#ffffff'
  const white = 1.05 / (luminance + 0.05)
  return white >= TEXT_MIN_CONTRAST || white >= (luminance + 0.05) / 0.05 ? '#ffffff' : '#000000'
}

/** WCAG AA for normal text. */
const SIDE_NAV_INK_MIN_CONTRAST = 4.5

/**
 * Foreground for glyphs, dividers and hover tints on the side nav: white on a
 * dark rail; on a light one, the theme's text color when it reads against the
 * rail itself, else black.
 */
export function sideNavInkColor(tokens: ThemeTokens): string {
  if (accentLabelColor(tokens.bgSideNav) === '#ffffff') return '#ffffff'
  return contrastRatio(tokens.textPrimary, tokens.bgSideNav) >= SIDE_NAV_INK_MIN_CONTRAST
    ? tokens.textPrimary
    : '#000000'
}

/**
 * The accent, darkened just enough for white text to meet WCAG AA on it: the
 * fill for count badges, which read as white numbers on every theme. Accents
 * that already pass come back unchanged.
 */
export function whiteLabelAccent(accent: string): string {
  const oklch = toOklch(accent)
  if (!oklch) return accent
  let fill = toHex(accent) ?? accent
  while (contrastRatio('#ffffff', fill) < 4.5 && oklch.l > 0) {
    oklch.l = Math.max(0, oklch.l - 0.01)
    fill = fromOklch(oklch)
  }
  return fill
}

// Derived rather than persisted so existing custom themes gain readable labels too.
export function accentLabelVars(tokens: ThemeTokens): Record<string, string> {
  return {
    '--me-accent-label': accentLabelColor(tokens.accent),
    '--me-accent-hover-label': accentLabelColor(tokens.accentHover),
    '--me-sidenav-ink': sideNavInkColor(tokens),
    '--me-accent-badge': whiteLabelAccent(tokens.accent),
  }
}

function sanitizeTokens(raw: unknown): ThemeTokens | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null
  const obj = raw as Record<string, unknown>
  const out = {} as Record<keyof ThemeTokens, string>
  for (const key of THEME_TOKEN_KEYS) {
    const value = obj[key]
    if (typeof value !== 'string' || !value) return null
    out[key] = value
  }
  return out
}

/**
 * The surface paints menus, dialogs and tooltips over other content, so a
 * translucent one is composited over the background it would have shown.
 */
export function withOpaqueSurface(source: CustomThemeInput): CustomThemeInput {
  if ((parseColor(source.surface)?.a ?? 1) >= 1) return source
  return { ...source, surface: flatten(source.surface, source.bgApp) }
}

const SOURCE_COLOR_KEYS = ['bgApp', 'surface', 'sideNav', 'accent', 'text'] as const

function sanitizeSource(raw: unknown): CustomThemeInput | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null
  const obj = raw as Record<string, unknown>
  const appearance = obj.appearance
  if (appearance !== 'light' && appearance !== 'dark') return null
  const colors = {} as Record<(typeof SOURCE_COLOR_KEYS)[number], string>
  for (const key of SOURCE_COLOR_KEYS) {
    const value = obj[key]
    if (typeof value !== 'string' || !isValidColor(value)) return null
    colors[key] = value
  }
  return withOpaqueSurface({ appearance, ...colors })
}

/**
 * Shareable one-line form of a custom theme's editor inputs, e.g.
 * "light,#f0f2f1,#ffffff,#121a16,#0e7a58,#1b211e": appearance, then background,
 * surface, side navigation, accent and text as hex. The field order is the
 * format; a future layout would need a new leading marker.
 */
export function serializeThemeSource(source: CustomThemeInput): string {
  const colors = SOURCE_COLOR_KEYS.map((key) => toHex(source[key]) ?? source[key])
  return [source.appearance, ...colors].join(',')
}

/** Inverse of serializeThemeSource; null for anything that isn't exactly that shape. */
export function parseThemeSource(text: string): CustomThemeInput | null {
  const parts = text
    .trim()
    .split(',')
    .map((part) => part.trim())
  if (parts.length !== SOURCE_COLOR_KEYS.length + 1) return null
  const [appearance, ...colors] = parts
  const hex = colors.map((color) => (color.startsWith('#') ? toHex(color) : null))
  if (hex.some((color) => color === null)) return null
  return sanitizeSource({
    appearance: appearance.toLowerCase(),
    ...Object.fromEntries(SOURCE_COLOR_KEYS.map((key, index) => [key, hex[index]])),
  })
}

/**
 * Validate the persisted `custom_themes` JSON. Entries with a valid source but
 * missing/corrupt tokens, or tokens derived from a surface that has since been
 * flattened, are re-derived; entries without a valid source are dropped. Follows the sanitizeKanbanBoards convention: null = unusable input.
 */
export function sanitizeCustomThemes(raw: unknown): CustomTheme[] | null {
  if (!Array.isArray(raw)) return null
  const out: CustomTheme[] = []
  const seen = new Set<string>()
  for (const item of raw) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) continue
    const obj = item as Record<string, unknown>
    if (typeof obj.id !== 'string' || !isCustomThemeId(obj.id) || seen.has(obj.id)) continue
    const source = sanitizeSource(obj.source)
    if (!source) continue
    const name = typeof obj.name === 'string' && obj.name.trim() ? obj.name.trim() : 'Custom theme'
    const flattened = source.surface !== (obj.source as Record<string, unknown>).surface
    const tokens = (flattened ? null : sanitizeTokens(obj.tokens)) ?? deriveThemeTokens(source)
    seen.add(obj.id)
    out.push({ id: obj.id, name, appearance: source.appearance, tokens, source })
  }
  return out
}
