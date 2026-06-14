import { describe, expect, it } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { ErrorBoundary } from './ErrorBoundary'

// Note: React error boundaries only engage during client rendering, not SSR
// (renderToStaticMarkup rethrows), and the repo has no DOM-renderer test infra.
// So the throw-path is covered via getDerivedStateFromError — the pure hook React
// calls to swap in the fallback — rather than a full mount.
describe('ErrorBoundary', () => {
  it('renders children when they do not throw', () => {
    const html = renderToStaticMarkup(
      <ErrorBoundary>
        <span>healthy</span>
      </ErrorBoundary>,
    )
    expect(html).toContain('healthy')
    expect(html).not.toContain('Something went wrong')
  })

  it('getDerivedStateFromError captures the error into state', () => {
    const err = new Error('boom')
    expect(ErrorBoundary.getDerivedStateFromError(err)).toEqual({ error: err })
  })
})
