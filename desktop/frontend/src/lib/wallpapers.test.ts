import { describe, expect, it } from 'bun:test'
import { sanitizeChatWallpaper, wallpaperCss } from './wallpapers'

describe('sanitizeChatWallpaper', () => {
  it('accepts known presets and Meron-owned custom URLs', () => {
    expect(sanitizeChatWallpaper({ kind: 'preset', presetId: 'dots' })).toEqual({
      kind: 'preset',
      presetId: 'dots',
    })
    expect(sanitizeChatWallpaper({ kind: 'preset', presetId: 'meron-lines' })).toEqual({
      kind: 'preset',
      presetId: 'aurora',
    })
    expect(sanitizeChatWallpaper({ kind: 'custom', url: '/media/wallpapers/acct/one.png' })).toEqual({
      kind: 'custom',
      url: '/media/wallpapers/acct/one.png',
    })
  })

  it('drops unknown presets and external custom URLs', () => {
    expect(sanitizeChatWallpaper({ kind: 'preset', presetId: 'missing' })).toBeNull()
    expect(sanitizeChatWallpaper({ kind: 'custom', url: 'https://example.com/a.png' })).toBeNull()
    expect(sanitizeChatWallpaper(null)).toBeNull()
  })
})

describe('wallpaperCss', () => {
  it('falls back to the default wallpaper for invalid values', () => {
    expect(wallpaperCss(undefined).className).toBe('wallpaper-preset-plain')
    expect(wallpaperCss({ kind: 'preset', presetId: 'missing' }).className).toBe('wallpaper-preset-plain')
  })

  it('uses cover-style custom image backgrounds', () => {
    const css = wallpaperCss({ kind: 'custom', url: '/media/wallpapers/acct/one.png' })
    expect(css.className).toBe('me-wallpaper-custom')
    expect(css.style?.backgroundImage).toContain('/media/wallpapers/acct/one.png')
  })
})
