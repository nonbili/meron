import { describe, expect, it } from 'bun:test'
import { resolveInlineCids } from './html'

describe('resolveInlineCids', () => {
  it('rewrites cid refs to data URLs from the matching attachments', () => {
    const html =
      '<p>hi</p><img src="cid:meron-image-1-a@meron" alt="a.png"><p>mid</p><img src="cid:meron-image-2-b@meron" alt="b.png">'
    const out = resolveInlineCids(html, [
      { inlineId: 'meron-image-1-a@meron', mime: 'image/png', data: 'AAAA' },
      { inlineId: 'meron-image-2-b@meron', mime: 'image/jpeg', data: 'BBBB' },
    ])
    expect(out).toBe(
      '<p>hi</p><img src="data:image/png;base64,AAAA" alt="a.png"><p>mid</p><img src="data:image/jpeg;base64,BBBB" alt="b.png">',
    )
  })

  it('ignores attachments without an inlineId and leaves unmatched cids alone', () => {
    const html = '<img src="cid:known@meron"><img src="cid:unknown@meron">'
    const out = resolveInlineCids(html, [
      { mime: 'application/pdf', data: 'CCCC' },
      { inlineId: 'known@meron', mime: 'image/png', data: 'DDDD' },
    ])
    expect(out).toBe('<img src="data:image/png;base64,DDDD"><img src="cid:unknown@meron">')
  })
})
