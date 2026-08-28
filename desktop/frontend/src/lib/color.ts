// Minimal color math for theme derivation (lib/themes.ts). Works on sRGB
// values parsed from hex / rgb() / rgba() strings — enough for the custom
// theme editor, which only emits hex inputs.

export type Rgba = { r: number; g: number; b: number; a: number }

const clamp255 = (n: number) => Math.min(255, Math.max(0, Math.round(n)))
const clamp01 = (n: number) => Math.min(1, Math.max(0, n))

/** Parse "#rgb", "#rrggbb", "#rrggbbaa", "rgb(...)" or "rgba(...)". */
export function parseColor(input: string): Rgba | null {
  const str = input.trim()

  const hex = str.match(/^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i)?.[1]
  if (hex) {
    if (hex.length === 3) {
      return {
        r: parseInt(hex[0] + hex[0], 16),
        g: parseInt(hex[1] + hex[1], 16),
        b: parseInt(hex[2] + hex[2], 16),
        a: 1,
      }
    }
    return {
      r: parseInt(hex.slice(0, 2), 16),
      g: parseInt(hex.slice(2, 4), 16),
      b: parseInt(hex.slice(4, 6), 16),
      a: hex.length === 8 ? parseInt(hex.slice(6, 8), 16) / 255 : 1,
    }
  }

  const fn = str.match(/^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*(?:,\s*([\d.]+)\s*)?\)$/i)
  if (fn) {
    return {
      r: clamp255(Number(fn[1])),
      g: clamp255(Number(fn[2])),
      b: clamp255(Number(fn[3])),
      a: fn[4] === undefined ? 1 : clamp01(Number(fn[4])),
    }
  }

  return null
}

/** Serialize as "#rrggbb", or "rgba(r, g, b, a)" when alpha < 1. */
export function formatColor({ r, g, b, a }: Rgba): string {
  if (a >= 1) {
    const to2 = (n: number) => clamp255(n).toString(16).padStart(2, '0')
    return `#${to2(r)}${to2(g)}${to2(b)}`
  }
  return `rgba(${clamp255(r)}, ${clamp255(g)}, ${clamp255(b)}, ${Math.round(clamp01(a) * 100) / 100})`
}

/** Linear blend of two colors: weight 0 -> a, 1 -> b. */
export function mix(colorA: string, colorB: string, weight: number): string {
  const a = parseColor(colorA)
  const b = parseColor(colorB)
  if (!a || !b) return colorA
  const w = clamp01(weight)
  return formatColor({
    r: a.r + (b.r - a.r) * w,
    g: a.g + (b.g - a.g) * w,
    b: a.b + (b.b - a.b) * w,
    a: a.a + (b.a - a.a) * w,
  })
}

/** Blend toward white by `amount` (0..1). */
export function lighten(color: string, amount: number): string {
  return mix(color, '#ffffff', amount)
}

/** Blend toward black by `amount` (0..1). */
export function darken(color: string, amount: number): string {
  return mix(color, '#000000', amount)
}

/** Same color with the given alpha. */
export function withAlpha(color: string, alpha: number): string {
  const parsed = parseColor(color)
  if (!parsed) return color
  return formatColor({ ...parsed, a: clamp01(alpha) })
}

/** Perceived luminance 0..1 (WCAG-ish, good enough to pick contrasting text). */
export function luminance(color: string): number {
  const parsed = parseColor(color)
  if (!parsed) return 0
  return (0.2126 * parsed.r + 0.7152 * parsed.g + 0.0722 * parsed.b) / 255
}

export function isValidColor(input: string): boolean {
  return parseColor(input) !== null
}
