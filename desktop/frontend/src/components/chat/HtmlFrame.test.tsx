import { afterEach, expect, it } from 'bun:test'
import { cleanup, fireEvent, render } from '@testing-library/react'
import { StrictMode } from 'react'
import { HtmlFrame } from './HtmlFrame'

afterEach(cleanup)

it('readies a document once after parsing, before subresources finish', () => {
  const readied: Document[] = []
  const { container } = render(
    <HtmlFrame
      html="<p>Message</p>"
      title="Message"
      onReady={(doc) => {
        readied.push(doc)
      }}
      readyWhenParsed
    />,
  )
  const frame = container.querySelector('iframe')!
  const doc = new DOMParser().parseFromString('<p>Partial</p>', 'text/html')
  let state: DocumentReadyState = 'loading'
  Object.defineProperty(doc, 'readyState', { get: () => state })
  Object.defineProperty(frame, 'contentDocument', { get: () => doc })
  readied.length = 0
  fireEvent.load(frame)
  expect(readied).toHaveLength(0)
  state = 'interactive'
  doc.body.innerHTML = '<p>Complete message</p><img src="pending.png">'
  fireEvent.load(frame)
  expect(readied).toEqual([doc])
  state = 'complete'
  fireEvent.load(frame)
  expect(readied).toEqual([doc])
})

it('reinstalls the ready hook after StrictMode effect replay', () => {
  const prototype = HTMLIFrameElement.prototype
  const original = Object.getOwnPropertyDescriptor(prototype, 'contentDocument')!
  const doc = new DOMParser().parseFromString('<p>Already parsed</p>', 'text/html')
  Object.defineProperty(doc, 'readyState', { value: 'complete' })
  Object.defineProperty(prototype, 'contentDocument', { configurable: true, get: () => doc })
  let installed = 0
  let disposed = 0
  try {
    const { container } = render(
      <StrictMode>
        <HtmlFrame
          html="<p>Already parsed</p>"
          title="Message"
          onReady={() => {
            installed++
            return () => {
              disposed++
            }
          }}
          readyWhenParsed
        />
      </StrictMode>,
    )
    expect(installed).toBe(2)
    expect(disposed).toBe(1)
    fireEvent.load(container.querySelector('iframe')!)
    expect(installed - disposed).toBe(1)
    cleanup()
    expect(installed).toBe(disposed)
  } finally {
    cleanup()
    Object.defineProperty(prototype, 'contentDocument', original)
  }
})

it('waits once for DOMContentLoaded without polling a hanging image', () => {
  const request = window.requestAnimationFrame
  const cancel = window.cancelAnimationFrame
  const frames: FrameRequestCallback[] = []
  window.requestAnimationFrame = (callback) => {
    frames.push(callback)
    return frames.length
  }
  window.cancelAnimationFrame = () => {}
  try {
    const readied: Document[] = []
    const { container } = render(
      <HtmlFrame
        html="<p>Message</p>"
        title="Message"
        onReady={(doc) => {
          readied.push(doc)
        }}
        readyWhenParsed
      />,
    )
    const frame = container.querySelector('iframe')!
    // The old document survives more than one animation frame before navigation.
    const old = new DOMParser().parseFromString('<p>Outgoing</p>', 'text/html')
    Object.defineProperty(old, 'readyState', { value: 'complete' })
    let current = old
    Object.defineProperty(frame, 'contentDocument', { get: () => current })
    frames.shift()!(0)
    frames.shift()!(16)
    expect(frames).toHaveLength(1)
    const doc = new DOMParser().parseFromString(frame.getAttribute('srcdoc')!, 'text/html')
    let state: DocumentReadyState = 'loading'
    Object.defineProperty(doc, 'readyState', { get: () => state })
    current = doc
    readied.length = 0
    const callback = frames.shift()!
    callback(0)
    expect(frames).toHaveLength(0)
    expect(readied).toHaveLength(0)
    state = 'interactive'
    doc.body.innerHTML = '<p>Parsed message</p><img src="hanging.png">'
    fireEvent(doc, new Event('DOMContentLoaded'))
    expect(readied).toEqual([doc])
    expect(frames).toHaveLength(0)
    fireEvent.load(frame)
    expect(readied).toEqual([doc])
  } finally {
    cleanup()
    window.requestAnimationFrame = request
    window.cancelAnimationFrame = cancel
  }
})

it('does not poll when the current srcdoc was already parsed before setup', () => {
  const prototype = HTMLIFrameElement.prototype
  const original = Object.getOwnPropertyDescriptor(prototype, 'contentDocument')!
  const request = window.requestAnimationFrame
  const cancel = window.cancelAnimationFrame
  let doc: Document | null = null
  let frames = 0
  let readied = 0
  window.requestAnimationFrame = () => {
    frames++
    return frames
  }
  window.cancelAnimationFrame = () => {}
  Object.defineProperty(prototype, 'contentDocument', {
    configurable: true,
    get(this: HTMLIFrameElement) {
      doc ??= new DOMParser().parseFromString(this.getAttribute('srcdoc')!, 'text/html')
      Object.defineProperty(doc, 'readyState', { configurable: true, value: 'interactive' })
      return doc
    },
  })
  try {
    render(
      <HtmlFrame
        html="<p>Already parsed</p>"
        title="Message"
        readyWhenParsed
        onReady={() => {
          readied++
        }}
      />,
    )
    expect(readied).toBe(1)
    expect(frames).toBe(0)
  } finally {
    cleanup()
    Object.defineProperty(prototype, 'contentDocument', original)
    window.requestAnimationFrame = request
    window.cancelAnimationFrame = cancel
  }
})

it('rewires a reader at load after an earlier interactive ready callback', () => {
  const readied: DocumentReadyState[] = []
  const disposed: DocumentReadyState[] = []
  const { container, rerender } = render(
    <HtmlFrame
      html="<p>Message</p>"
      title="Message"
      onReady={(doc) => {
        readied.push(doc.readyState)
        return () => {
          disposed.push(doc.readyState)
        }
      }}
    />,
  )
  const frame = container.querySelector('iframe')!
  const doc = new DOMParser().parseFromString('<p>Message</p><img src="pending.png">', 'text/html')
  let state: DocumentReadyState = 'interactive'
  Object.defineProperty(doc, 'readyState', { get: () => state })
  Object.defineProperty(frame, 'contentDocument', { get: () => doc })
  readied.length = 0
  disposed.length = 0
  rerender(
    <HtmlFrame
      html="<p>Message with image</p>"
      title="Message"
      onReady={(doc) => {
        readied.push(doc.readyState)
        return () => {
          disposed.push(doc.readyState)
        }
      }}
    />,
  )
  expect(readied).toEqual(['interactive'])
  disposed.length = 0
  state = 'complete'
  fireEvent.load(frame)
  expect(readied).toEqual(['interactive', 'complete'])
  expect(disposed).toEqual(['complete'])
})

it("forwards cached and remote pictures to the app's image context menu", () => {
  const { container } = render(
    <HtmlFrame html="<p>Message</p>" title="Message" forwardContextMenu="except-images" allowRemoteImages />,
  )
  const frame = container.querySelector('iframe')!
  const doc = new DOMParser().parseFromString(
    `<p>Message</p><img id="cached" src="${window.location.origin}/media/msg/0.png"><img id="remote" src="https://example.com/a.png">`,
    'text/html',
  )
  Object.defineProperty(doc, 'readyState', { value: 'complete' })
  Object.defineProperty(frame, 'contentDocument', { get: () => doc })
  fireEvent.load(frame)
  const forwarded: (string | null | undefined)[] = []
  frame.addEventListener('contextmenu', (event) => forwarded.push((event as { meronImageKey?: string }).meronImageKey))
  const images: unknown[] = []
  frame.addEventListener('contextmenu', (event) => images.push((event as any).meronImage))

  const cached = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
  doc.getElementById('cached')!.dispatchEvent(cached)
  expect(cached.defaultPrevented).toBe(true)
  expect(forwarded).toEqual(['msg/0.png'])

  const remote = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
  doc.getElementById('remote')!.dispatchEvent(remote)
  expect(remote.defaultPrevented).toBe(true)
  expect(forwarded).toEqual(['msg/0.png', null])
  expect(images[1]).toEqual({ key: null, url: 'https://example.com/a.png', filename: 'a.png' })
})

it('never lets a message link navigate its frame', () => {
  const opened: string[] = []
  const runtime = (window as any).runtime
  ;(window as any).runtime = { BrowserOpenURL: (url: string) => opened.push(url) }
  try {
    const { container } = render(
      <HtmlFrame
        html="<p>Message</p>"
        title="Message"
        // The reader opens the gallery for any picture, linked or not.
        onFrameClick={(event) => !!(event.target as Element).closest('img')}
      />,
    )
    const frame = container.querySelector('iframe')!
    // The base stands in for the app's own `wails://` origin on Linux and macOS.
    const doc = new DOMParser().parseFromString(
      `<base href="wails://wails/">
       <a id="media" href="/media/acct/inbox/1/0.html">open</a>
       <a id="sibling" href="0.html">open</a>
       <map><area id="area" href="/media/acct/inbox/1/0.html"></map>
       <a href="/media/acct/inbox/1/0.html"><img id="image" src="/media/acct/inbox/1/1.png"></a>
       <a id="fragment" href="#top">top</a>
       <a id="external" href="https://example.com/x">site</a>`,
      'text/html',
    )
    Object.defineProperty(doc, 'readyState', { value: 'complete' })
    Object.defineProperty(frame, 'contentDocument', { get: () => doc })
    fireEvent.load(frame)

    const click = (id: string, type = 'click', button = 0) => {
      const event = new MouseEvent(type, { bubbles: true, cancelable: true, button })
      doc.getElementById(id)!.dispatchEvent(event)
      return event.defaultPrevented
    }
    expect(click('media')).toBe(true)
    expect(click('media', 'auxclick', 1)).toBe(true)
    expect(click('sibling')).toBe(true)
    expect(click('area')).toBe(true)
    expect(click('image')).toBe(true)
    expect(click('fragment')).toBe(false)
    expect(click('external')).toBe(true)
    expect(opened).toEqual(['https://example.com/x'])
  } finally {
    ;(window as any).runtime = runtime
  }
})
