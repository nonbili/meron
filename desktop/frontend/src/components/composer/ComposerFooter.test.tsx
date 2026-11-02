import { afterEach, describe, expect, it } from 'bun:test'
import { cleanup, render, screen } from '@testing-library/react'
import { ComposerFooter } from './ComposerFooter'

const noop = () => {}

afterEach(cleanup)

function renderFooter(saveStatus: 'idle' | 'saving' | 'saved' | 'error') {
  return render(
    <ComposerFooter
      rich={false}
      sending={false}
      saveStatus={saveStatus}
      canSend
      onPickFiles={noop}
      onPickInlineImages={noop}
      onToggleRich={noop}
      onDiscard={noop}
      onSubmit={noop}
    />,
  )
}

describe('ComposerFooter save status', () => {
  it('shows "saved" as an icon with the words as its tooltip, not as text', () => {
    const { container } = renderFooter('saved')
    const status = screen.getByRole('status', { name: 'Saved to Server' })
    expect(status.getAttribute('title')).toBe('Saved to Server')
    expect(status.querySelector('svg')).not.toBeNull()
    expect(container.textContent).not.toContain('Saved to Server')
  })

  it('shows "saving" as an icon with the words as its tooltip, not as text', () => {
    const { container } = renderFooter('saving')
    const status = screen.getByRole('status', { name: 'Saving draft...' })
    expect(status.getAttribute('title')).toBe('Saving draft...')
    expect(status.querySelector('svg')).not.toBeNull()
    expect(container.textContent).not.toContain('Saving')
  })

  it('shows no saved status while idle', () => {
    renderFooter('idle')
    expect(screen.queryByRole('status')).toBeNull()
  })
})
