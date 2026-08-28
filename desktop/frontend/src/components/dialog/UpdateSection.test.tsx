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
})
