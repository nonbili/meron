import { describe, expect, it } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { ConversationSubject } from './ConversationSubject'

describe('ConversationSubject', () => {
  it('exposes the complete subject in an accessible tooltip', () => {
    const subject = '[nonbili/NouTube] [feature request] adding seeking slider, a closing button, and more controls'
    const html = renderToStaticMarkup(
      <ConversationSubject subject={subject} copyLabel="Copy subject" onCopy={() => undefined} />,
    )

    const tooltipId = html.match(/aria-describedby="([^"]+)"/)?.[1]
    expect(tooltipId).toBeTruthy()
    expect(html).toContain(`id="${tooltipId}"`)
    expect(html).toContain('role="tooltip"')
    expect(html).toContain('aria-label="Copy subject"')
    expect(html.match(new RegExp(subject.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'g'))).toHaveLength(2)
    expect(html).not.toContain('title="Copy subject"')
  })

  it('paints the tooltip with the opaque surface colour, not the translucent raised one', () => {
    const html = renderToStaticMarkup(
      <ConversationSubject subject="Subject" copyLabel="Copy subject" onCopy={() => undefined} />,
    )
    const tooltipClass = html.match(/<span[^>]*role="tooltip"[^>]*>/)?.[0].match(/class="([^"]+)"/)?.[1] ?? ''
    expect(tooltipClass).toContain('bg-chats')
    expect(tooltipClass).not.toContain('bg-raised')
  })
})
