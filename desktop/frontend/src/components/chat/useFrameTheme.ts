import { useMemo } from 'react'
import { useValue } from '@legendapp/state/react'
import { resolveThemeDef, settings$ } from '../../states/settings'
import { bubbleThemeFromTokens, readerThemeFromTokens, type BubbleTheme, type ReaderTheme } from './frameTheme'
import type { ThemeDef } from '../../lib/themes'

/**
 * The active theme, subscribed to. Both inputs to `resolveThemeDef` are read as
 * observables: editing the active custom theme must repaint the body frames the
 * same way switching themes does.
 */
function useActiveTheme(): ThemeDef {
  const themeId = useValue(settings$.themeId)
  const customThemes = useValue(settings$.customThemes)
  return useMemo(() => resolveThemeDef(), [themeId, customThemes])
}

/** The palette a reader body frame should paint with. */
export function useReaderTheme(): ReaderTheme {
  const def = useActiveTheme()
  return useMemo(() => readerThemeFromTokens(def.appearance, def.tokens), [def])
}

/** The palette a bubble body frame should paint with, for the bubble it sits in. */
export function useBubbleTheme(outgoing: boolean): BubbleTheme {
  const def = useActiveTheme()
  return useMemo(() => bubbleThemeFromTokens(def.appearance, def.tokens, outgoing), [def, outgoing])
}
