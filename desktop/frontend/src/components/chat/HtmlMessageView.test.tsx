import { afterEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import '../../states/compose'
import { HtmlMessageView } from './HtmlMessageView'

const attachments = [
  { key: null, url: 'https://cdn.example/clip.mp4', filename: 'clip.mp4', mime: 'video/mp4', size: 0 },
]

afterEach(() => {
  cleanup()
})

describe('reader videos', () => {
  for (const viewMode of ['html', 'plain'] as const) {
    it(`renders feed video attachments in ${viewMode} mode`, () => {
      const { container } = render(
        <HtmlMessageView
          scrollKey="feed-video"
          title="Feed"
          html="<p>Caption</p>"
          text="Caption"
          attachments={attachments}
          viewMode={viewMode}
          allowRemote
        />,
      )
      expect(container.querySelector('button[title="Open in external player"]')).not.toBeNull()
      expect(container.querySelector('video')).toBeNull()
    })
  }

  it('keeps one self-sizing frame when attachments change without changing the HTML', () => {
    const props = {
      scrollKey: 'switch-layout',
      title: 'Feed',
      html: '<p>Caption</p>',
      text: 'Caption',
      viewMode: 'html' as const,
      allowRemote: true,
    }
    const view = render(<HtmlMessageView {...props} />)
    const bodyFrame = view.container.querySelector('iframe')!

    view.rerender(<HtmlMessageView {...props} attachments={attachments} />)
    expect(view.container.querySelector('iframe')).toBe(bodyFrame)
  })

  it('scrolls the header with the body', () => {
    const view = render(
      <HtmlMessageView
        scrollKey="header"
        title="Feed"
        html="<p>Caption</p>"
        text="Caption"
        viewMode="html"
        header={<h1>Subject</h1>}
      />,
    )
    const scroller = view.container.querySelector('iframe')!.parentElement!.parentElement!
    expect(scroller.contains(view.getByText('Subject'))).toBe(true)
  })

  it('anchors hovered URLs outside the attachment scroll container', () => {
    const view = render(
      <HtmlMessageView
        scrollKey="hover"
        title="Feed"
        html="<p>Caption</p>"
        text="Caption"
        attachments={attachments}
        viewMode="html"
        allowRemote
      />,
    )
    const frame = view.container.querySelector('iframe')!
    const doc = document.implementation.createHTMLDocument('Feed')
    Object.defineProperty(doc, 'defaultView', { value: window })
    doc.body.innerHTML = '<a href="https://example.com/article">Article</a>'
    Object.defineProperty(frame, 'contentDocument', { value: doc })
    Object.defineProperty(frame, 'contentWindow', { value: window })
    fireEvent.load(frame)
    const scrollContainer = frame.parentElement!.parentElement!
    scrollContainer.scrollTop = 100
    fireEvent.scroll(scrollContainer)
    fireEvent.mouseOver(doc.querySelector('a')!)

    const preview = view.getByText('https://example.com/article').parentElement!
    expect(scrollContainer.contains(preview)).toBe(false)
    expect(preview.parentElement).toBe(scrollContainer.parentElement)
  })

  for (const scenario of ['iframe growth', 'attachment growth', 'user scroll while pending'] as const) {
    it(`handles scroll restoration after ${scenario}`, async () => {
      const props = {
        scrollKey: `late-height-${scenario}`,
        title: 'Feed',
        html: '<p>Caption</p>',
        text: 'Caption',
        attachments: [{ key: 'image.png', filename: 'image.png', mime: 'image/png', size: 0, url: null }],
        viewMode: 'html' as const,
        allowRemote: true,
      }
      const first = render(<HtmlMessageView {...props} />)
      const firstScroller = first.container.querySelector('iframe')!.parentElement!.parentElement!
      firstScroller.scrollTop = 500
      fireEvent.scroll(firstScroller)
      first.unmount()

      const originalObserver = globalThis.ResizeObserver
      const observers = new Map<Element, () => void>()
      globalThis.ResizeObserver = class {
        private callback: () => void
        constructor(callback: () => void) {
          this.callback = callback
        }
        observe(element: Element) {
          observers.set(element, this.callback)
        }
        unobserve(element: Element) {
          observers.delete(element)
        }
        disconnect() {}
      } as unknown as typeof ResizeObserver
      try {
        const view = render(<HtmlMessageView {...props} />)
        const frame = view.container.querySelector('iframe')!
        const content = frame.parentElement!
        const scroller = content.parentElement!
        let scrollTop = 0
        let attachmentHeight = 0
        Object.defineProperty(scroller, 'scrollTop', {
          get: () => scrollTop,
          set: (top: number) => {
            scrollTop = Math.min(top, Math.max(0, parseFloat(frame.style.height) + attachmentHeight - 200))
          },
        })
        const doc = document.implementation.createHTMLDocument('Feed')
        Object.defineProperty(doc, 'defaultView', { value: window })
        let contentHeight = 300
        doc.body.getBoundingClientRect = () => new DOMRect(0, 0, 600, contentHeight)
        doc.documentElement.getBoundingClientRect = doc.body.getBoundingClientRect
        Object.defineProperty(frame, 'contentDocument', { value: doc })
        Object.defineProperty(frame, 'contentWindow', { value: window })
        fireEvent.load(frame)
        await waitFor(() => expect(scroller.scrollTop).toBe(100))
        // The clamped programmatic scroll must preserve the saved target.
        fireEvent.scroll(scroller)

        if (scenario === 'user scroll while pending') {
          // No wheel/key/pointer event: e.g. a native scrollbar drag.
          scroller.scrollTop = 40
          fireEvent.scroll(scroller)
        }
        if (scenario === 'attachment growth') {
          attachmentHeight = 700
          fireEvent.load(content.querySelector('img')!)
        } else {
          contentHeight = 1000
          act(() => observers.get(doc.body)!())
        }
        act(() => observers.get(content)!())
        expect(scroller.scrollTop).toBe(scenario === 'user scroll while pending' ? 40 : 500)
        if (scenario === 'attachment growth') expect(frame.style.height).toBe('300px')
        if (scenario === 'user scroll while pending') {
          view.unmount()
          const reopened = render(<HtmlMessageView {...props} />)
          const reopenedFrame = reopened.container.querySelector('iframe')!
          Object.defineProperty(reopenedFrame, 'contentDocument', { value: doc })
          Object.defineProperty(reopenedFrame, 'contentWindow', { value: window })
          fireEvent.load(reopenedFrame)
          await waitFor(() => expect(reopenedFrame.parentElement!.parentElement!.scrollTop).toBe(40))
        }
      } finally {
        cleanup()
        globalThis.ResizeObserver = originalObserver
      }
    })
  }

  it('holds remote video attachments back until allowed', () => {
    const { container } = render(
      <HtmlMessageView scrollKey="blocked" title="Feed" text="Caption" attachments={attachments} viewMode="plain" />,
    )
    expect(container.querySelector('button')).toBeNull()
  })
})
