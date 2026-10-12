import { describe, expect, it } from 'bun:test'
import { fireEvent, render, screen } from '@testing-library/react'
import { InfoTip } from './InfoTip'

describe('InfoTip', () => {
  it('paints the bubble with the opaque surface colour, not the translucent raised one', () => {
    const { container } = render(<InfoTip hint="Keep Meron running in the system tray." />)
    fireEvent.mouseEnter(container.firstElementChild!)
    const tooltip = screen.getByRole('tooltip')
    expect(tooltip.textContent).toBe('Keep Meron running in the system tray.')
    expect(tooltip.className).toContain('bg-chats')
    expect(tooltip.className).not.toContain('bg-raised')
  })
})
