import { afterEach, describe, expect, it } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { EMPTY_UPDATE_STATUS, update$ } from '../../states/update'
import { UpdateSection } from './UpdateSection'

describe('UpdateSection', () => {
  afterEach(() => {
    update$.status.set(EMPTY_UPDATE_STATUS)
  })

  it('shows an installation failure while keeping the staged update retryable', () => {
    update$.status.set({
      ...EMPTY_UPDATE_STATUS,
      state: 'ready',
      channel: 'appimage',
      supported: true,
      latestVersion: '0.1.13',
      error: 'install directory is not writable',
    })

    const html = renderToStaticMarkup(<UpdateSection />)
    expect(html).toContain('Update failed')
    expect(html).toContain('install directory is not writable')
    expect(html).toContain('Restart &amp; install')
  })

  it('shows an update error as readable text instead of only a tooltip', () => {
    const message = 'Windows update failed: elevation cancelled'
    update$.status.set({ ...EMPTY_UPDATE_STATUS, supported: true, state: 'error', error: message })
    const html = renderToStaticMarkup(<UpdateSection />)
    expect(html).toMatch(/<p\b[^>]*>Windows update failed: elevation cancelled<\/p>/)
  })

  it('keeps the installation explanation visible when a check finds an update', () => {
    const message = 'Windows update failed: elevation cancelled'
    update$.status.set({
      ...EMPTY_UPDATE_STATUS,
      supported: true,
      state: 'available',
      latestVersion: '0.4.2',
      installError: message,
    })
    const html = renderToStaticMarkup(<UpdateSection />)
    expect(html).toMatch(/<p\b[^>]*>Windows update failed: elevation cancelled<\/p>/)
    expect(html).toContain('Download')
  })
})
