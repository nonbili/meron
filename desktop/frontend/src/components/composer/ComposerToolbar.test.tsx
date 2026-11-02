import { afterEach, describe, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { Editor } from '@tiptap/core'
import type { Editor as ReactEditor } from '@tiptap/react'
import { StarterKit } from '@tiptap/starter-kit'
import { ComposerToolbar } from './ComposerToolbar'

let editor: Editor | null = null

function renderToolbar() {
  editor = new Editor({ extensions: [StarterKit] })
  return render(<ComposerToolbar editor={editor as unknown as ReactEditor} onSetLink={() => {}} />)
}

afterEach(() => {
  cleanup()
  editor?.destroy()
  editor = null
})

describe('ComposerToolbar Markdown help', () => {
  it('keeps the Markdown examples out of the toolbar until the icon is clicked', () => {
    const { container } = renderToolbar()
    expect(container.textContent).not.toContain('**bold**')
    expect(screen.queryByRole('dialog')).toBeNull()

    fireEvent.click(screen.getByTitle('Markdown'))

    const help = screen.getByRole('dialog')
    expect(help.textContent).toContain('Type these and they format as you write')
    expect(help.textContent).toContain('**bold**')
    expect(help.textContent).toContain('# heading')
    expect(help.textContent).toContain('- list')
  })

  it('closes on a second click, on Escape and on a click elsewhere', () => {
    renderToolbar()
    const button = screen.getByTitle('Markdown')

    fireEvent.click(button)
    fireEvent.click(button)
    expect(screen.queryByRole('dialog')).toBeNull()

    fireEvent.click(button)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByRole('dialog')).toBeNull()

    fireEvent.click(button)
    fireEvent.pointerDown(document.body)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('leaves focus in the editor when the icon or the popover is pressed', () => {
    renderToolbar()
    const button = screen.getByTitle('Markdown')

    // fireEvent.mouseDown returns false once the default, moving focus, is prevented.
    expect(fireEvent.mouseDown(button)).toBe(false)
    fireEvent.click(button)
    expect(fireEvent.mouseDown(screen.getByRole('dialog'))).toBe(false)
  })
})
