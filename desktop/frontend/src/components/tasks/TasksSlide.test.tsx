import { afterEach, beforeEach, describe, expect, it, jest } from 'bun:test'
import { act, cleanup, render } from '@testing-library/react'
import { PANE_ANIMATION_MS } from '../kanban/KanbanConversationPane'
import { TasksSlide } from './TasksSlide'

const slide = (open: boolean) => (
  <TasksSlide open={open}>
    <p>tasks</p>
  </TasksSlide>
)

const originalMatchMedia = (globalThis as any).matchMedia
let reducedMotion = false

describe('TasksSlide', () => {
  beforeEach(() => {
    jest.useFakeTimers()
    reducedMotion = false
    ;(globalThis as any).matchMedia = () => ({
      matches: reducedMotion,
      addEventListener: () => {},
      removeEventListener: () => {},
    })
  })

  afterEach(() => {
    cleanup()
    jest.useRealTimers()
    ;(globalThis as any).matchMedia = originalMatchMedia
  })

  const root = (view: ReturnType<typeof render>) =>
    view.container.querySelector('[data-pane-phase]') as HTMLElement | null

  it('slides open, then settles without the animation', () => {
    const view = render(slide(false))
    expect(root(view)).toBeNull()

    view.rerender(slide(true))
    expect(root(view)?.dataset.panePhase).toBe('entering')
    expect(root(view)?.className).toContain('animate-pane-open')
    expect(view.queryByText('tasks')).not.toBeNull()

    act(() => {
      jest.advanceTimersByTime(PANE_ANIMATION_MS)
    })
    expect(root(view)?.dataset.panePhase).toBe('open')
    expect(root(view)?.className).not.toContain('animate-pane')
  })

  it('keeps the list on screen while it slides out, then unmounts', () => {
    const view = render(slide(true))
    expect(root(view)?.dataset.panePhase).toBe('open')

    view.rerender(slide(false))
    expect(root(view)?.dataset.panePhase).toBe('exiting')
    expect(root(view)?.className).toContain('animate-pane-close')
    expect(view.queryByText('tasks')).not.toBeNull()

    act(() => {
      jest.advanceTimersByTime(PANE_ANIMATION_MS)
    })
    expect(root(view)).toBeNull()
  })

  it('opens and closes at once with reduced motion', () => {
    reducedMotion = true
    const view = render(slide(false))
    view.rerender(slide(true))
    expect(root(view)?.dataset.panePhase).toBe('open')
    view.rerender(slide(false))
    expect(root(view)).toBeNull()
  })
})
