import { beforeEach, describe, expect, it } from 'bun:test'
import { compose$ } from '../../states/composeState'
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
})
