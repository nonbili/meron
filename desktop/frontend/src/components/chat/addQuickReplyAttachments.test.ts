import { beforeEach, describe, expect, it } from 'bun:test'
import { compose$ } from '../../states/composeState'
import { ui$ } from '../../states/ui'
import { addQuickReplyAttachments } from './useQuickReply'

// FileReader reports asynchronously.
async function attachmentsSettled(count: number) {
  for (let i = 0; i < 50 && compose$.composerAttachments.get().length < count; i++) {
    await new Promise((resolve) => setTimeout(resolve, 5))
  }
  return compose$.composerAttachments.get()
}

describe('addQuickReplyAttachments', () => {
  beforeEach(() => {
    compose$.composerAttachments.set([])
  })

  it('adds dropped files to the reply as attachments, images included', async () => {
    addQuickReplyAttachments([
      new File(['report'], 'report.pdf', { type: 'application/pdf' }),
      new File(['png'], 'photo.png', { type: 'image/png' }),
    ])
    const attachments = await attachmentsSettled(2)
    expect(attachments.map((a) => [a.filename, a.mime, a.size])).toEqual([
      ['report.pdf', 'application/pdf', 6],
      ['photo.png', 'image/png', 3],
    ])
  })

  it('leaves out files still being read when the reader moves to another conversation', async () => {
    const before = ui$.selectedThread.peek()
    ui$.selectedThread.set('thread-a')
    addQuickReplyAttachments([new File(['report'], 'report.pdf', { type: 'application/pdf' })])
    ui$.selectedThread.set('thread-b')
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(compose$.composerAttachments.get()).toEqual([])
    ui$.selectedThread.set(before)
  })

  it('leaves out what goes over the size limits, counting what is already attached', async () => {
    const sized = (name: string, megabytes: number) =>
      Object.defineProperty(new File(['x'], name), 'size', { value: megabytes * 1024 * 1024 })
    compose$.composerAttachments.set([{ id: '1', filename: 'old.bin', mime: '', size: 90 * 1024 * 1024, data: '' }])
    ui$.toast.set('')
    addQuickReplyAttachments([sized('huge.bin', 101), sized('fits.bin', 50), sized('over-total.bin', 50)])
    const attachments = await attachmentsSettled(2)
    await new Promise((resolve) => setTimeout(resolve, 30))
    expect(attachments.map((a) => a.filename)).toEqual(['old.bin', 'fits.bin'])
    expect(ui$.toast.get()).not.toBe('')
    ui$.toast.set('')
  })

  it('counts files still being read, so two goes cannot each fill the message', async () => {
    const sized = (name: string) => Object.defineProperty(new File(['x'], name), 'size', { value: 90 * 1024 * 1024 })
    addQuickReplyAttachments([sized('first.bin')])
    addQuickReplyAttachments([sized('second.bin')])
    await attachmentsSettled(1)
    await new Promise((resolve) => setTimeout(resolve, 30))
    expect(compose$.composerAttachments.get().map((a) => a.filename)).toEqual(['first.bin'])
    // And the room is free again once they have landed.
    compose$.composerAttachments.set([])
    addQuickReplyAttachments([sized('third.bin')])
    expect((await attachmentsSettled(1)).map((a) => a.filename)).toEqual(['third.bin'])
    ui$.toast.set('')
  })
})
