import { ConversationViewContext } from './conversationView'
import { thread$ } from '../../states/thread'
import { afterEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import '../../states/compose'
import { BubbleHtmlFrame } from './BubbleHtmlFrame'

const originalObserver = globalThis.IntersectionObserver

afterEach(() => {
  cleanup()
  globalThis.IntersectionObserver = originalObserver
})

it('shows a cached remount as soon as its document is themed', async () => {
  let visibility: (near: boolean) => void = () => {}
  globalThis.IntersectionObserver = class {
    constructor(callback: IntersectionObserverCallback) {
      visibility = (near) =>
        callback([{ isIntersecting: near } as IntersectionObserverEntry], this as unknown as IntersectionObserver)
    }
    observe() {}
    disconnect() {}
  } as unknown as typeof IntersectionObserver

  const { container } = render(<BubbleHtmlFrame html="<p>A remounted newsletter</p>" />)
  act(() => visibility(true))
  const first = container.querySelector('iframe')!
  const ready = (frame: HTMLIFrameElement) => {
    const doc = new DOMParser().parseFromString(frame.getAttribute('srcdoc')!, 'text/html')
    Object.defineProperty(doc, 'readyState', { value: 'interactive' })
    Object.defineProperty(doc.documentElement, 'clientWidth', { value: 300 })
    Object.defineProperty(frame, 'contentDocument', { get: () => doc })
    fireEvent.load(frame)
  }
  ready(first)
  await waitFor(() => expect(first.style.visibility).toBe('visible'))
  act(() => visibility(false))
  expect(container.querySelector('iframe')).toBeNull()
  act(() => visibility(true))
  const second = container.querySelector('iframe')!
  expect(second).not.toBe(first)
  expect(second.style.visibility).toBe('hidden')
  ready(second)
  expect(second.style.visibility).toBe('hidden')
  await waitFor(() => expect(second.style.visibility).toBe('visible'))
})

it('shows readable text even when the asynchronous theme walk throws', async () => {
  globalThis.IntersectionObserver = undefined as unknown as typeof IntersectionObserver
  const { container } = render(<BubbleHtmlFrame html="<p>A message with a styling failure</p>" />)
  const frame = container.querySelector('iframe')!
  const doc = new DOMParser().parseFromString(frame.getAttribute('srcdoc')!, 'text/html')
  Object.defineProperty(doc, 'readyState', { value: 'interactive' })
  Object.defineProperty(doc.documentElement, 'clientWidth', { value: 300 })
  Object.defineProperty(frame, 'contentDocument', { get: () => doc })
  const setProperty = doc.documentElement.style.setProperty.bind(doc.documentElement.style)
  let failed = false
  doc.documentElement.style.setProperty = (name, value, priority) => {
    if (!failed && name.endsWith('-text')) {
      failed = true
      throw new Error('Computed theme failed')
    }
    setProperty(name, value, priority)
  }
  fireEvent.load(frame)
  await waitFor(() => expect(frame.style.visibility).toBe('visible'))
  expect(failed).toBe(true)
  expect(doc.documentElement.style.getPropertyValue('color-scheme')).not.toBe('')
})

it('preserves blocked remote and intrinsic-size-free SVG boxes while releasing missing local images', async () => {
  globalThis.IntersectionObserver = undefined as unknown as typeof IntersectionObserver
  const { container } = render(
    <BubbleHtmlFrame html='<img id="remote" src="https://example.com/photo.png" width="600" height="400"><img id="svg" src="/media/a/1/vector.svg" width="600" height="400"><img id="local" src="/media/a/1/photo.jfif" width="600" height="400">' />,
  )
  const frame = container.querySelector('iframe')!
  const doc = new DOMParser().parseFromString(frame.getAttribute('srcdoc')!, 'text/html')
  Object.defineProperty(doc, 'readyState', { value: 'interactive' })
  Object.defineProperty(doc.documentElement, 'clientWidth', { value: 300 })
  Object.defineProperty(frame, 'contentDocument', { get: () => doc })
  for (const image of doc.querySelectorAll('img')) {
    Object.defineProperty(image, 'complete', { value: true })
    Object.defineProperty(image, 'naturalWidth', { value: 0 })
  }
  fireEvent.load(frame)
  await waitFor(() => expect(frame.style.visibility).toBe('visible'))
  const remote = doc.getElementById('remote')!
  const svg = doc.getElementById('svg')!
  const local = doc.getElementById('local')!
  fireEvent.error(remote)
  expect(remote.getAttribute('width')).toBe('600')
  expect(svg.getAttribute('width')).toBe('600')
  expect(local.hasAttribute('width')).toBe(false)
  fireEvent.error(svg)
  expect(svg.hasAttribute('width')).toBe(false)
  fireEvent.load(svg)
  expect(svg.getAttribute('width')).toBe('600')
})

it('retries a delayed image error after the frame consumes media recovery', async () => {
  globalThis.IntersectionObserver = undefined as unknown as typeof IntersectionObserver
  const html = '<img src="/media/a/1/photo.png">'
  const { container, rerender } = render(<BubbleHtmlFrame html={html} mediaMissing={1} />)
  const frame = container.querySelector('iframe')!
  const doc = new DOMParser().parseFromString(frame.getAttribute('srcdoc')!, 'text/html')
  Object.defineProperty(doc, 'readyState', { value: 'interactive' })
  Object.defineProperty(doc.documentElement, 'clientWidth', { value: 300 })
  Object.defineProperty(frame, 'contentDocument', { get: () => doc })
  const image = doc.querySelector('img')!
  Object.defineProperty(image, 'complete', { value: false })
  Object.defineProperty(image, 'naturalWidth', { value: 0 })
  let requests = 0
  const setAttribute = image.setAttribute.bind(image)
  image.setAttribute = (name, value) => {
    if (name === 'src') requests++
    setAttribute(name, value)
  }
  fireEvent.load(frame)
  await waitFor(() => expect(frame.style.visibility).toBe('visible'))
  rerender(<BubbleHtmlFrame html={html} mediaMissing={0} />)
  expect(container.querySelector('iframe')).toBe(frame)
  expect(requests).toBe(0)
  fireEvent.error(image)
  expect(requests).toBe(1)
  fireEvent.error(image)
  expect(requests).toBe(1)
})

it('keeps existing hidden frames and defers mounting new ones until activation', () => {
  let visibility: (near: boolean) => void = () => {}
  globalThis.IntersectionObserver = class {
    constructor(callback: IntersectionObserverCallback) {
      visibility = (near) =>
        callback([{ isIntersecting: near } as IntersectionObserverEntry], this as unknown as IntersectionObserver)
    }
    observe() {}
    disconnect() {}
  } as unknown as typeof IntersectionObserver
  const frame = (active: boolean) => (
    <ConversationViewContext value={{ state: thread$, active, scope: 'test' }}>
      <BubbleHtmlFrame html="<p>Keep only viewport frames</p>" />
    </ConversationViewContext>
  )
  const view = render(frame(false))
  act(() => visibility(true))
  expect(view.container.querySelector('iframe') === null).toBe(true)
  view.rerender(frame(true))
  act(() => visibility(true))
  const loaded = view.container.querySelector('iframe')
  expect(loaded !== null).toBe(true)
  view.rerender(frame(false))
  act(() => visibility(false))
  expect(view.container.querySelector('iframe') === loaded).toBe(true)
  view.rerender(frame(true))
  act(() => visibility(false))
  expect(view.container.querySelector('iframe') === null).toBe(true)
})
