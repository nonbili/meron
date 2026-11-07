import { afterEach, describe, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import '../../states/compose'
import type { MessageTab } from '../../types'
import { ReaderTabView } from './ReaderTabView'

afterEach(cleanup)

describe('reader tab scroll isolation', () => {
  for (const viewMode of ['html', 'plain'] as const) {
    it(`starts new ${viewMode} tabs at the top and restores each tab's saved position`, async () => {
      const tab = (id: string): MessageTab => ({
        id,
        kind: 'reader',
        messageId: id,
        threadId: id,
        subject: id,
        from: 'Sender',
        body: 'Caption',
        bodyHtml: '<p>Caption</p>',
        attachments: [{ key: 'clip.mp4', filename: 'clip.mp4', mime: 'video/mp4', size: 0, url: null }],
        viewMode,
      })
      const a = tab(`scroll-a-${viewMode}`)
      const b = tab(`scroll-b-${viewMode}`)
      const view = render(<ReaderTabView tab={a} />)
      const scroller = () => view.container.querySelector<HTMLElement>('.overflow-y-auto')!
      const loadFrame = () => {
        const frame = view.container.querySelector('iframe')
        if (!frame) return
        const doc = document.implementation.createHTMLDocument('Feed')
        Object.defineProperty(doc, 'defaultView', { value: window })
        doc.body.getBoundingClientRect = () => new DOMRect(0, 0, 600, 1200)
        doc.documentElement.getBoundingClientRect = doc.body.getBoundingClientRect
        Object.defineProperty(frame, 'contentDocument', { value: doc })
        Object.defineProperty(frame, 'contentWindow', { value: window })
        fireEvent.load(frame)
      }

      loadFrame()
      const firstScroller = scroller()
      firstScroller.scrollTop = 400
      fireEvent.scroll(firstScroller)

      view.rerender(<ReaderTabView tab={b} />)
      expect(scroller()).not.toBe(firstScroller)
      expect(scroller().scrollTop).toBe(0)
      loadFrame()
      scroller().scrollTop = 150
      fireEvent.scroll(scroller())

      view.rerender(<ReaderTabView tab={a} />)
      loadFrame()
      await waitFor(() => expect(scroller().scrollTop).toBe(400))

      view.rerender(<ReaderTabView tab={b} />)
      loadFrame()
      await waitFor(() => expect(scroller().scrollTop).toBe(150))
    })
  }
})

// A reader tab shows the same message as the conversation, so it says so too
// when that message couldn't be decrypted.
describe('reader tab encryption', () => {
  it('shows why an encrypted message could not be opened', () => {
    const tab: MessageTab = {
      id: 'reader-crypto',
      kind: 'reader',
      messageId: 'reader-crypto',
      threadId: 'reader-crypto',
      subject: '...',
      from: 'Sender',
      body: '',
      bodyHtml: '',
      viewMode: 'plain',
      message: {
        id: 'reader-crypto',
        account_id: 'a',
        from_name: 'Sender',
        from_addr: 'sender@example.com',
        crypto: { encrypted: true, signed: false, problem: 'no_key' },
      } as unknown as MessageTab['message'],
    }
    const view = render(<ReaderTabView tab={tab} />)
    expect(view.getByRole('alert')).toBeTruthy()
  })
})
