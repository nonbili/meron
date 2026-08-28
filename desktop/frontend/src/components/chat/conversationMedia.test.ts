import { describe, expect, it } from 'bun:test'
import type { Attachment, Message } from '../../types'
import { buildThreadMedia } from './conversationMedia'

function message(id: string, attachments: Attachment[]): Message {
  return {
    id,
    account_id: 'account-1',
    folder_id: 'inbox',
    thread_id: 'thread-1',
    from_name: 'Sender',
    from_addr: 'sender@example.com',
    to: 'me@example.com',
    subject: 'Subject',
    preview: '',
    body: '',
    date: 0,
    unread: false,
    starred: false,
    has_attachments: attachments.length > 0,
    attachments,
  }
}

describe('buildThreadMedia', () => {
  it('keeps each panel item linked to its source message', () => {
    const messages = [
      message('older-message', [
        { filename: 'older.png', mime: 'image/png', size: 10, key: 'older.png' },
        { filename: 'older.txt', mime: 'text/plain', size: 20, key: 'older.txt' },
      ]),
      message('newer-message', [
        { filename: 'newer.mp4', mime: 'video/mp4', size: 30, key: 'newer.mp4' },
        { filename: 'newer.pdf', mime: 'application/pdf', size: 40, key: 'newer.pdf' },
      ]),
    ]

    const { mediaItems, fileItems } = buildThreadMedia(messages, [], {}, [])

    expect(mediaItems.map(({ filename, messageId, galleryIndex }) => ({ filename, messageId, galleryIndex }))).toEqual([
      { filename: 'newer.mp4', messageId: 'newer-message', galleryIndex: 1 },
      { filename: 'older.png', messageId: 'older-message', galleryIndex: 0 },
    ])
    expect(fileItems.map(({ filename, messageId }) => ({ filename, messageId }))).toEqual([
      { filename: 'newer.pdf', messageId: 'newer-message' },
      { filename: 'older.txt', messageId: 'older-message' },
    ])
  })
})
