import { afterEach, describe, expect, it } from 'bun:test'
import { cleanup, render, screen } from '@testing-library/react'
import { ComposerFooter } from './ComposerFooter'

const noop = () => {}

afterEach(cleanup)

function footer(saveStatus: 'idle' | 'saving' | 'saved' | 'error') {
  return (
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
    />
  )
}

describe('ComposerFooter save status', () => {
  it('shows "saved" as an icon with the words as its tooltip, and only screen readers get the text', () => {
    render(footer('saved'))
    expect(screen.getByTitle('Saved to Server').querySelector('svg')).not.toBeNull()
    const status = screen.getByRole('status')
    expect(status.textContent).toBe('Saved to Server')
    expect(status.className).toContain('sr-only')
  })

  it('shows "saving" as an icon with the words as its tooltip, and only screen readers get the text', () => {
    render(footer('saving'))
    expect(screen.getByTitle('Saving draft...').querySelector('svg')).not.toBeNull()
    const status = screen.getByRole('status')
    expect(status.textContent).toBe('Saving draft...')
    expect(status.className).toContain('sr-only')
  })

  it('shows no saved status while idle, but keeps the live region mounted', () => {
    const { container } = render(footer('idle'))
    expect(screen.getByRole('status').textContent).toBe('')
    expect(container.querySelector('[title="Saved to Server"], [title="Saving draft..."]')).toBeNull()
  })

  it('changes the text inside one live region, so screen readers announce it', () => {
    const { rerender } = render(footer('idle'))
    const status = screen.getByRole('status')
    rerender(footer('saving'))
    expect(screen.getByRole('status')).toBe(status)
    expect(status.textContent).toBe('Saving draft...')
    rerender(footer('saved'))
    expect(screen.getByRole('status')).toBe(status)
    expect(status.textContent).toBe('Saved to Server')
  })

  it('keeps the icon element across saving and saved, so an open tooltip survives', () => {
    const { rerender } = render(footer('saving'))
    const icon = screen.getByTitle('Saving draft...')
    rerender(footer('saved'))
    expect(screen.getByTitle('Saved to Server')).toBe(icon)
  })
})
