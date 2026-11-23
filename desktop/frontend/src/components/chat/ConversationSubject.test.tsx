import { describe, expect, it } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { ConversationSubject } from './ConversationSubject'

describe('ConversationSubject', () => {
  it('shows the complete subject, wrapped rather than truncated', () => {
    const subject = '[nonbili/NouTube] [feature request] adding seeking slider, a closing button, and more controls'
    const html = renderToStaticMarkup(
      <ConversationSubject subject={subject} copyLabel="Copy subject" copiedLabel="Subject copied" />,
    )
    expect(html).toContain(subject)
    expect(html).toContain('break-words')
    expect(html).not.toContain('truncate')
    expect(html).toContain('title="Copy subject"')
    expect(html).toContain('aria-label="Copy subject"')
    expect(html).toContain(`<h2 class="min-w-0" aria-label="${subject}">`)
  })

  it('puts the subtitle under the subject', () => {
    const html = renderToStaticMarkup(
      <ConversationSubject subject="Feed" subtitle="feeds.example.com" copyLabel="Copy" copiedLabel="Copied" />,
    )
    expect(html.indexOf('Feed')).toBeLessThan(html.indexOf('feeds.example.com'))
  })
})
