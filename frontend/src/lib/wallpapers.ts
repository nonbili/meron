import type { CSSProperties } from 'react'
import type { ChatWallpaper } from '../types'

export type WallpaperPreset = {
  id: string
  name: string
  previewClass: string
  className: string
}

export const WALLPAPER_PRESETS: WallpaperPreset[] = [
  // Clean / Geometric presets
  { id: 'plain', name: 'Plain', previewClass: 'wallpaper-preset-plain', className: 'wallpaper-preset-plain' },
  { id: 'doodle', name: 'Doodle', previewClass: 'wallpaper-preset-doodle', className: 'wallpaper-preset-doodle' },
  { id: 'dots', name: 'Linear Dots', previewClass: 'wallpaper-preset-dots', className: 'wallpaper-preset-dots' },
  { id: 'grid', name: 'Classic Grid', previewClass: 'wallpaper-preset-grid', className: 'wallpaper-preset-grid' },
  {
    id: 'stripes',
    name: 'Diagonal Stripes',
    previewClass: 'wallpaper-preset-stripes',
    className: 'wallpaper-preset-stripes',
  },
  {
    id: 'hexagon',
    name: 'Hexagon Grid',
    previewClass: 'wallpaper-preset-hexagon',
    className: 'wallpaper-preset-hexagon',
  },
  {
    id: 'isometric',
    name: 'Isometric Cubes',
    previewClass: 'wallpaper-preset-isometric',
    className: 'wallpaper-preset-isometric',
  },
  { id: 'waves', name: 'Flowing Waves', previewClass: 'wallpaper-preset-waves', className: 'wallpaper-preset-waves' },
  {
    id: 'nordic',
    name: 'Nordic Pattern',
    previewClass: 'wallpaper-preset-nordic',
    className: 'wallpaper-preset-nordic',
  },

  // Rich / Artistic / Photographic presets
  {
    id: 'topography',
    name: 'Topography',
    previewClass: 'wallpaper-preset-topography',
    className: 'wallpaper-preset-topography',
  },
  {
    id: 'constellation',
    name: 'Constellation',
    previewClass: 'wallpaper-preset-constellation',
    className: 'wallpaper-preset-constellation',
  },
  { id: 'aurora', name: 'Aurora', previewClass: 'wallpaper-preset-aurora', className: 'wallpaper-preset-aurora' },
  { id: 'nebula', name: 'Nebula', previewClass: 'wallpaper-preset-nebula', className: 'wallpaper-preset-nebula' },
  { id: 'sunset', name: 'Sunset Glow', previewClass: 'wallpaper-preset-sunset', className: 'wallpaper-preset-sunset' },
  { id: 'forest', name: 'Forest Mist', previewClass: 'wallpaper-preset-forest', className: 'wallpaper-preset-forest' },
  { id: 'desert', name: 'Desert Dunes', previewClass: 'wallpaper-preset-desert', className: 'wallpaper-preset-desert' },
  { id: 'ocean', name: 'Tranquil Ocean', previewClass: 'wallpaper-preset-ocean', className: 'wallpaper-preset-ocean' },
  {
    id: 'mountain',
    name: 'Mountain Range',
    previewClass: 'wallpaper-preset-mountain',
    className: 'wallpaper-preset-mountain',
  },
  { id: 'breeze', name: 'Soft Breeze', previewClass: 'wallpaper-preset-breeze', className: 'wallpaper-preset-breeze' },
  {
    id: 'galaxy',
    name: 'Spiral Galaxy',
    previewClass: 'wallpaper-preset-galaxy',
    className: 'wallpaper-preset-galaxy',
  },
  {
    id: 'shapes',
    name: 'Abstract Shapes',
    previewClass: 'wallpaper-preset-shapes',
    className: 'wallpaper-preset-shapes',
  },
  {
    id: 'sakura',
    name: 'Sakura Watercolor',
    previewClass: 'wallpaper-preset-sakura',
    className: 'wallpaper-preset-sakura',
  },
  {
    id: 'vintage',
    name: 'Vintage Parchment',
    previewClass: 'wallpaper-preset-vintage',
    className: 'wallpaper-preset-vintage',
  },
  {
    id: 'raindrops',
    name: 'Raindrops',
    previewClass: 'wallpaper-preset-raindrops',
    className: 'wallpaper-preset-raindrops',
  },
  { id: 'marble', name: 'Sleek Marble', previewClass: 'wallpaper-preset-marble', className: 'wallpaper-preset-marble' },
  {
    id: 'cyberpunk',
    name: 'Cyberpunk Grid',
    previewClass: 'wallpaper-preset-cyberpunk',
    className: 'wallpaper-preset-cyberpunk',
  },
  {
    id: 'matrix',
    name: 'Digital Matrix',
    previewClass: 'wallpaper-preset-matrix',
    className: 'wallpaper-preset-matrix',
  },
  {
    id: 'autumn',
    name: 'Autumn Leaves',
    previewClass: 'wallpaper-preset-autumn',
    className: 'wallpaper-preset-autumn',
  },
  {
    id: 'nightsky',
    name: 'Celestial Night',
    previewClass: 'wallpaper-preset-nightsky',
    className: 'wallpaper-preset-nightsky',
  },
]

export function sanitizeChatWallpaper(raw: unknown): ChatWallpaper | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null
  const obj = raw as Record<string, unknown>
  if (obj.kind === 'preset') {
    let presetId = typeof obj.presetId === 'string' ? obj.presetId : ''
    // Migrate old preset IDs to new premium designs
    if (presetId === 'meron-lines') presetId = 'aurora'
    if (presetId === 'soft-waves') presetId = 'topography'

    return WALLPAPER_PRESETS.some((preset) => preset.id === presetId) ? { kind: 'preset', presetId } : null
  }
  if (obj.kind === 'custom') {
    const url = typeof obj.url === 'string' ? obj.url.trim() : ''
    return url.startsWith('/media/wallpapers/') ? { kind: 'custom', url } : null
  }
  return null
}

export function wallpaperCss(wallpaper: ChatWallpaper | null | undefined): {
  className: string
  style?: CSSProperties
} {
  const clean = sanitizeChatWallpaper(wallpaper)
  if (!clean) return { className: 'wallpaper-preset-plain' }
  if (clean.kind === 'preset') {
    const preset = WALLPAPER_PRESETS.find((item) => item.id === clean.presetId)
    return { className: preset?.className ?? 'wallpaper-preset-plain' }
  }
  return {
    className: 'me-wallpaper-custom',
    style: {
      backgroundImage: `linear-gradient(var(--me-wallpaper-overlay), var(--me-wallpaper-overlay)), url("${clean.url}")`,
    },
  }
}
