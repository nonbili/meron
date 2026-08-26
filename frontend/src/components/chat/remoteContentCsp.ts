// The sidecar bakes each HTML body with a CSP `<meta>` whose `img-src`/`media-src`
// decide whether remote content loads (see `prepare_html`). That decision is made
// when the body is read, so a change made afterwards has to rewrite the baked
// policy here rather than wait for a refetch — loosening it for a reveal ("show
// images in this message", or allowing the sender while the thread is open), and
// tightening it when the trust is taken back, since a body read while the sender
// was allowed carries a permissive policy until the thread is read again.
const REMOTE_SOURCES = ['http:', 'https:']

/** Source-list tokens that let remote content through. */
const REMOTE_TOKENS = new Set([...REMOTE_SOURCES, '*', 'http://*', 'https://*'])

/** Add `http:`/`https:` to the `img-src` and `media-src` of every CSP meta in
 *  `doc`. Other directives (crucially `script-src 'none'`) are left alone. */
export function allowRemoteContent(doc: Document) {
  rewriteCspMetas(doc, allowRemoteInCsp)
}

/** Rewrite `html`'s baked CSP to the caller's current remote-content decision.
 *  For views that hand a whole document to an iframe rather than building one
 *  (the reader tab), so a reveal or a withdrawn allowance reaches them too. */
export function applyRemoteContentPolicy(html: string, allowRemote: boolean): string {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  if (allowRemote) allowRemoteContent(doc)
  else blockRemoteContent(doc)
  return doc.documentElement.outerHTML
}

/** Drop `http:`/`https:`/`*` from the `img-src` and `media-src` of every CSP meta
 *  in `doc`, so a body baked while its sender was allowed stops loading remote
 *  content the moment that allowance is withdrawn. */
export function blockRemoteContent(doc: Document) {
  rewriteCspMetas(doc, blockRemoteInCsp)
}

function rewriteCspMetas(doc: Document, rewrite: (csp: string) => string) {
  const metas = doc.querySelectorAll('meta[http-equiv="Content-Security-Policy" i]')
  for (const meta of metas) {
    const content = meta.getAttribute('content')
    if (!content) continue
    meta.setAttribute('content', rewrite(content))
  }
}

/** The same rewrite on a raw CSP string, for callers that hold the header text. */
export function allowRemoteInCsp(csp: string): string {
  return csp
    .split(';')
    .map((directive) => {
      const trimmed = directive.trim()
      const name = trimmed.split(/\s+/)[0]?.toLowerCase()
      if (name !== 'img-src' && name !== 'media-src') return directive
      const missing = REMOTE_SOURCES.filter((source) => !trimmed.toLowerCase().includes(source))
      return missing.length ? `${directive} ${missing.join(' ')}` : directive
    })
    .join(';')
}

/** The tightening counterpart of [`allowRemoteInCsp`], on a raw CSP string. */
export function blockRemoteInCsp(csp: string): string {
  return csp
    .split(';')
    .map((directive) => {
      const parts = directive.trim().split(/\s+/)
      const name = parts[0]?.toLowerCase()
      if (name !== 'img-src' && name !== 'media-src') return directive
      const kept = parts.slice(1).filter((source) => !REMOTE_TOKENS.has(source.toLowerCase()))
      // Keep the directive's own spacing, and never leave it source-less: an
      // empty source list is invalid, where `'none'` is the block we want.
      const lead = directive.slice(0, directive.length - directive.trimStart().length)
      return `${lead}${name} ${kept.length ? kept.join(' ') : "'none'"}`
    })
    .join(';')
}
