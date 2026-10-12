import { afterEach, beforeEach, expect, it } from 'bun:test'
import { cleanup, render } from '@testing-library/react'
import { compose$ } from '../../states/composeState'
import { QuickReplyComposer } from './QuickReplyComposer'

// The test relies on an empty reply (its send button is the disabled one), and
// other test files can leave text or attachments behind.
beforeEach(() => {
  compose$.composer.set('')
  compose$.composerAttachments.set([])
})

afterEach(cleanup)

// The reply text gets the box's full width: it sits on its own row, and the
// attach, full-editor and send buttons share a toolbar row below it instead of
// flanking it.
it('gives the reply text its own row, with the buttons in a row below it', () => {
  const view = render(<QuickReplyComposer />)
  const textarea = view.container.querySelector('textarea')!
  // An empty reply can't be sent, so the send button is the disabled one.
  const toolbar = view.container.querySelector('button[disabled]')!.parentElement!
  expect(toolbar.contains(textarea)).toBe(false)
  expect(textarea.compareDocumentPosition(toolbar) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  expect(toolbar.querySelectorAll('button')).toHaveLength(3)
})
