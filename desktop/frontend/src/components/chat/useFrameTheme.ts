import { useValue } from '@legendapp/state/react'
import { useMemo } from 'react'
import { useActiveTheme } from '../../lib/useThemeChoice'
import { settings$ } from '../../states/settings'
import {
  bubbleThemeFromTokens,
  readerThemeFromTokens,
  type BubbleTheme,
  type FrameSurface,
  type ReaderTheme,
} from './frameTheme'

/** The palette a reader body frame should paint with. */
export function useReaderTheme(): ReaderTheme {
  const def = useActiveTheme()
  const darkenStyled = useValue(settings$.darkMessageBodies)
  return useMemo(() => readerThemeFromTokens(def.appearance, def.tokens, darkenStyled), [def, darkenStyled])
}

/** The palette a bubble body frame should paint with, for the bubble or row it sits in. */
export function useBubbleTheme(outgoing: boolean, surface: FrameSurface = 'bubble'): BubbleTheme {
  const def = useActiveTheme()
  const darkenStyled = useValue(settings$.darkMessageBodies)
  return useMemo(
    () => bubbleThemeFromTokens(def.appearance, def.tokens, outgoing, darkenStyled, surface),
    [def, outgoing, darkenStyled, surface],
  )
}
