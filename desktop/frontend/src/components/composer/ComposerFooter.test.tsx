import { describe, expect, it } from 'bun:test'
import { render, screen } from '@testing-library/react'
import { ComposerFooter } from './ComposerFooter'

const noop = () => {}

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

  it('shows no saved status while idle', () => {
    renderFooter('idle')
    expect(screen.queryByRole('status')).toBeNull()
  })
})
