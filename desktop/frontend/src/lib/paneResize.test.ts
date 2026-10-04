import { afterEach, beforeEach, describe, expect, it, spyOn } from 'bun:test'
import type { PointerEvent as ReactPointerEvent } from 'react'
import { settings$ } from '../states/settings'
import { startKanbanResize } from './paneResize'

const originalViewport = Object.getOwnPropertyDescriptor(window, 'innerWidth')
let originalWidth: number

beforeEach(() => {
  originalWidth = settings$.kanbanPaneWidth.get()
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1000 })
})

afterEach(() => {
  window.dispatchEvent(new PointerEvent('pointerup'))
  settings$.kanbanPaneWidth.set(originalWidth)
  if (originalViewport) Object.defineProperty(window, 'innerWidth', originalViewport)
  else Reflect.deleteProperty(window, 'innerWidth')
})

function startDrag(right: number) {
  const pane = document.createElement('div')
  const handle = document.createElement('div')
  pane.append(handle)
  spyOn(pane, 'getBoundingClientRect').mockReturnValue(new DOMRect(right - 400, 0, 400, 600))
  startKanbanResize({
    currentTarget: handle,
    preventDefault() {},
  } as ReactPointerEvent<HTMLDivElement>)
}

describe('kanban pane resize', () => {
  for (const [layout, right] of [
    ['unframed', 1000],
    ['framed', 991],
    ['framed with Tasks', 651],
  ] as const) {
    it(`keeps the edge under the cursor in the ${layout} layout`, () => {
      startDrag(right)
      window.dispatchEvent(new PointerEvent('pointermove', { clientX: right - 400 }))
      expect(settings$.kanbanPaneWidth.get()).toBe(40)
    })
  }

  it('clamps the width and stops updating when the drag ends', () => {
    startDrag(991)
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 991 }))
    expect(settings$.kanbanPaneWidth.get()).toBe(25)
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 0 }))
    expect(settings$.kanbanPaneWidth.get()).toBe(60)
    window.dispatchEvent(new PointerEvent('pointerup'))
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 591 }))
    expect(settings$.kanbanPaneWidth.get()).toBe(60)
  })
})
