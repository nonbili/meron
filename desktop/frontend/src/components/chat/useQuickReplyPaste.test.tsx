import { afterEach, beforeEach, expect, it, spyOn } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { compose$ } from '../../states/composeState'
import { cancelQuickReplyDraftSave } from '../../states/quickReply'
import { ui$ } from '../../states/ui'
import { useQuickReply } from './useQuickReply'

function ReplyBox() {
  const { textareaRef, handleComposerKeyDown, handleComposerPaste } = useQuickReply()
  return <textarea ref={textareaRef} onKeyDown={handleComposerKeyDown} onPaste={handleComposerPaste} />
}

let restorePlatform: () => void

beforeEach(() => {
  const descriptors = ['platform', 'userAgent'].map(
    (key) => [key, Object.getOwnPropertyDescriptor(navigator, key)] as const,
  )
  restorePlatform = () => {
    for (const [key, descriptor] of descriptors) {
      if (descriptor) Object.defineProperty(navigator, key, descriptor)
      else delete (navigator as any)[key]
    }
  }
  cancelQuickReplyDraftSave()
  ui$.selectedThread.set('')
  compose$.composer.set('')
  compose$.composerAttachments.set([])
})

afterEach(() => {
  cleanup()
  cancelQuickReplyDraftSave()
  restorePlatform()
})

function setPlatform(platform: string) {
  Object.defineProperty(navigator, 'platform', { configurable: true, value: platform })
  Object.defineProperty(navigator, 'userAgent', { configurable: true, value: platform })
}

it('leaves macOS text paste to the browser without an asynchronous clipboard read', () => {
  setPlatform('MacIntel')
  const read = spyOn(navigator.clipboard, 'read').mockResolvedValue([])
  try {
    const view = render(<ReplyBox />)
    const body = view.container.querySelector('textarea')!
    fireEvent.keyDown(body, { key: 'v', metaKey: true })
    const pasted = fireEvent.paste(body, {
      clipboardData: { files: [], items: [], types: ['text/plain'], getData: () => 'Copied text' },
    })
    expect(pasted).toBe(true)
    expect(read).not.toHaveBeenCalled()
    expect(compose$.composerAttachments.peek()).toEqual([])
  } finally {
    read.mockRestore()
  }
})

it('attaches macOS pasted image files without an asynchronous clipboard read', async () => {
  setPlatform('MacIntel')
  const read = spyOn(navigator.clipboard, 'read').mockResolvedValue([])
  try {
    const view = render(<ReplyBox />)
    const body = view.container.querySelector('textarea')!
    fireEvent.keyDown(body, { key: 'v', metaKey: true })
    const pasted = fireEvent.paste(body, {
      clipboardData: {
        files: [new File(['image'], 'pasted.png', { type: 'image/png' })],
        items: [],
        types: ['Files'],
      },
    })
    expect(pasted).toBe(false)
    await waitFor(() => expect(compose$.composerAttachments.peek()).toHaveLength(1))
    expect(compose$.composerAttachments.peek()[0].filename).toBe('pasted.png')
    expect(read).not.toHaveBeenCalled()
  } finally {
    read.mockRestore()
  }
})

it('keeps the Linux asynchronous image paste fallback in quick reply', async () => {
  setPlatform('Linux x86_64')
  const read = spyOn(navigator.clipboard, 'read').mockResolvedValue([
    {
      types: ['image/png'],
      presentationStyle: 'unspecified',
      getType: async () => new Blob(['image'], { type: 'image/png' }),
    },
  ])
  try {
    const view = render(<ReplyBox />)
    fireEvent.keyDown(view.container.querySelector('textarea')!, { key: 'v', ctrlKey: true })
    await waitFor(() => expect(compose$.composerAttachments.peek()).toHaveLength(1))
    expect(read).toHaveBeenCalledTimes(1)
    expect(compose$.composerAttachments.peek()[0].mime).toBe('image/png')
  } finally {
    read.mockRestore()
  }
})
