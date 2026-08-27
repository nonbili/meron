import { describe, expect, it } from 'bun:test'
import type { Message } from '../../types'
import { plainHighlightTexts } from './messageHelpers'
import { messageMatchCount } from './threadSearchMatches'

function message(body: string): Message {
  return {
    id: 'message-1',
    account_id: 'account-1',
    folder_id: 'inbox',
    thread_id: 'thread-1',
    from_name: 'Sender',
    from_addr: 'sender@example.com',
    to: 'me@example.com',
    subject: 'Subject',
    preview: body,
    body,
    date: 0,
    unread: false,
    starred: false,
    has_attachments: false,
  }
}

describe('messageMatchCount', () => {
  it('counts every occurrence in a body, not the body itself', () => {
    expect(messageMatchCount(message('kitchen sink, kitchen door'), 'kitchen', false)).toBe(2)
  })

  it('ignores case and does not run matches together', () => {
    expect(messageMatchCount(message('Kitchen kitchen KITCHEN'), 'kitchen', false)).toBe(3)
    expect(messageMatchCount(message('aaaa'), 'aa', false)).toBe(2)
  })

  it('counts a match inside markdown emphasis, where the renderer marks one', () => {
    expect(messageMatchCount(message('a **bold kitchen** and `code kitchen`'), 'kitchen', false)).toBe(2)
  })

  it('leaves out what the renderer never highlights', () => {
    // A fenced code block renders raw, and a bare link renders shortened.
    const body = ['```', 'kitchen', '```', '', 'https://example.com/kitchen'].join('\n')
    expect(messageMatchCount(message(body), 'kitchen', false)).toBe(0)
  })

  it('reports nothing for a subject-only match, which has no mark of its own', () => {
    expect(messageMatchCount(message('nothing here'), 'subject', false)).toBe(0)
  })
})

describe('plainHighlightTexts', () => {
  it('yields the rendered text in render order, markers stripped', () => {
    expect(plainHighlightTexts('one **two** [three](https://example.com)').join('|')).toContain('two')
    expect(plainHighlightTexts('**bold**')).toEqual(['', 'bold', ''])
  })
})
