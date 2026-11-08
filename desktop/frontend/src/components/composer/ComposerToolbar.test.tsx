import { afterEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { Editor } from '@tiptap/core'
import type { Editor as ReactEditor } from '@tiptap/react'
import { StarterKit } from '@tiptap/starter-kit'
import { isMac } from '../../lib/shortcuts'
import { ComposerToolbar } from './ComposerToolbar'

let editor: Editor | null = null

function renderToolbar() {
  editor = new Editor({ extensions: [StarterKit] })
  return render(<ComposerToolbar editor={editor as unknown as ReactEditor} />)
}

afterEach(() => {
  cleanup()
  editor?.destroy()
  editor = null
})

describe('ComposerToolbar link dialog', () => {
  it('centers its backdrop within the composer and consumes composer shortcuts', () => {
    editor = new Editor({ extensions: [StarterKit] })
    let shortcuts = 0
    const view = render(
      <div data-composer="" style={{ position: 'relative' }} onKeyDown={() => shortcuts++}>
        <ComposerToolbar editor={editor as unknown as ReactEditor} />
      </div>,
    )
    fireEvent.click(screen.getByTitle('Link'))
    const dialog = screen.getByRole('dialog')
    const backdrop = dialog.parentElement!
    expect(backdrop.parentElement).toBe(view.container.querySelector('[data-composer]'))
    expect(backdrop.style.position).toBe('absolute')
    fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Enter', ctrlKey: true })
    expect(shortcuts).toBe(0)
  })

  function openLink() {
    renderToolbar()
    act(() => {
      editor!.commands.setContent('<p>Selected words and other words</p>')
      editor!.commands.setTextSelection({ from: 1, to: 15 })
    })
    fireEvent.click(screen.getByTitle('Link'))
  }

  it('applies a link to the preserved text selection when submitted', () => {
    openLink()
    const input = screen.getByRole('textbox', { name: 'Link URL' })
    expect(document.activeElement).toBe(input)
    fireEvent.change(input, { target: { value: ' https://example.com/ ' } })
    fireEvent.submit(input.closest('form')!)
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(editor!.getHTML()).toContain('href="https://example.com/"')
    expect(editor!.getHTML()).toContain('>Selected words</a> and other words')
  })

  it('wraps keyboard focus in both directions within the link dialog', () => {
    openLink()
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'https://example.com/' } })
    const close = screen.getByRole('button', { name: 'Close' })
    const save = screen.getByRole('button', { name: 'Save' })
    save.focus()
    expect(fireEvent.keyDown(save, { key: 'Tab' })).toBe(false)
    expect(document.activeElement).toBe(close)
    expect(fireEvent.keyDown(close, { key: 'Tab', shiftKey: true })).toBe(false)
    expect(document.activeElement).toBe(save)
  })

  it('runs plain-text undo and redo while keeping editing chords inside the dialog', () => {
    openLink()
    const input = screen.getByRole('textbox')
    const previous = document.execCommand
    const commands: string[] = []
    document.execCommand = (command) => {
      commands.push(command)
      return true
    }
    try {
      const modifier = isMac ? { metaKey: true } : { ctrlKey: true }
      expect(fireEvent.keyDown(input, { key: 'z', ...modifier })).toBe(false)
      expect(fireEvent.keyDown(input, { key: 'z', shiftKey: true, ...modifier })).toBe(false)
      expect(fireEvent.keyDown(input, { key: 'y', ...modifier })).toBe(false)
      expect(commands).toEqual(['undo', 'redo', 'redo'])
    } finally {
      document.execCommand = previous
    }
  })

  it('leaves the message unchanged when cancelled or dismissed with Escape', () => {
    openLink()
    const before = editor!.getHTML()
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'https://example.com/' } })
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(editor!.getHTML()).toBe(before)
    fireEvent.click(screen.getByTitle('Link'))
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(editor!.getHTML()).toBe(before)
  })

  it('prefills an existing link and can remove it while retaining the text', () => {
    renderToolbar()
    act(() => {
      editor!.commands.setContent('<p><a href="https://example.com/">Existing link</a></p>')
      editor!.commands.setTextSelection(4)
    })
    fireEvent.click(screen.getByTitle('Link'))
    expect((screen.getByRole('textbox') as HTMLInputElement).value).toBe('https://example.com/')
    fireEvent.click(screen.getByRole('button', { name: 'Remove link' }))
    expect(editor!.getHTML()).toBe('<p>Existing link</p>')
    expect(screen.queryByRole('dialog')).toBeNull()
  })
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
