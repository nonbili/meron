import { afterEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { ContactNotes } from './ContactNotes'
import { contactNotes$, loadContactNote, saveContactNote } from '../../states/contacts'

afterEach(() => {
  cleanup()
  contactNotes$.set({})
})
async function settle() {
  await act(async () => {
    await Promise.resolve()
  })
}

it('edits, previews Markdown, saves, and reloads by normalized address', async () => {
  const prefs: Record<string, string> = {}
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          if (command === 'contacts.noteGet') return { prefs }
          prefs[payload.key] = payload.value
          return { ok: true }
        },
      },
    },
  }
  const view = render(<ContactNotes addr=" Alice@Example.com " />)
  await settle()
  fireEvent.click(view.getByRole('button', { name: 'Add a note…' }))
  fireEvent.change(view.getByRole('textbox', { name: 'Notes' }), {
    target: { value: '# Background\n\n**Friend** from school\n\n- Likes tea' },
  })
  fireEvent.click(view.getByRole('button', { name: 'Preview' }))
  expect(view.getByRole('heading', { name: 'Background' })).toBeTruthy()
  expect(view.getByText('Friend').tagName).toBe('STRONG')
  expect(view.getByRole('listitem').textContent).toBe('Likes tea')
  fireEvent.click(view.getByRole('button', { name: 'Save' }))
  await settle()
  expect(prefs['contact_note:alice@example.com']).toContain('**Friend**')
  expect(view.queryByRole('textbox')).toBeNull()
  fireEvent.click(view.getByRole('button', { name: 'Edit' }))
  fireEvent.change(view.getByRole('textbox'), { target: { value: 'Unwanted edit' } })
  fireEvent.click(view.getByRole('button', { name: 'Cancel' }))
  expect(view.queryByRole('textbox')).toBeNull()
  expect(view.getByRole('heading', { name: 'Background' })).toBeTruthy()
  expect(contactNotes$['alice@example.com'].draft.peek()).toBe(prefs['contact_note:alice@example.com'])
  view.unmount()
  contactNotes$.set({})
  render(<ContactNotes addr="alice@example.com" />)
  await settle()
  expect(contactNotes$['alice@example.com'].saved.peek()).toBe(prefs['contact_note:alice@example.com'])
})

it('retains failed saves and unsaved drafts when switching contacts', async () => {
  let fail = true
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string) => {
          if (command === 'contacts.noteGet') return { prefs: {} }
          if (fail) throw new Error('offline')
          return { ok: true }
        },
      },
    },
  }
  const view = render(<ContactNotes key="alice" addr="alice@example.com" />)
  await settle()
  fireEvent.click(view.getByRole('button', { name: 'Add a note…' }))
  fireEvent.change(view.getByRole('textbox'), { target: { value: 'Private draft' } })
  fireEvent.click(view.getByRole('button', { name: 'Save' }))
  await settle()
  expect(view.getByRole('status').textContent).toContain('Could not save')
  expect(contactNotes$['alice@example.com'].saved.peek()).toBe('')
  view.rerender(<ContactNotes key="bob" addr="bob@example.com" />)
  await settle()
  view.rerender(<ContactNotes key="alice" addr="alice@example.com" />)
  await settle()
  expect((view.getByRole('textbox') as HTMLTextAreaElement).value).toBe('Private draft')
  fail = false
  fireEvent.click(view.getByRole('button', { name: 'Save' }))
  await settle()
  expect(contactNotes$['alice@example.com'].saved.peek()).toBe('Private draft')
})

it('does not execute HTML or load remote images in Markdown', async () => {
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async () => ({
          prefs: {
            'contact_note:alice@example.com':
              '<script>alert(1)</script>\n\n![tracking](https://example.com/pixel)\n\n[unsafe](javascript:alert(1))',
          },
        }),
      },
    },
  }
  const view = render(<ContactNotes addr="alice@example.com" />)
  await settle()
  expect(view.container.querySelector('script')).toBeNull()
  expect(view.container.querySelector('img')).toBeNull()
  expect(view.container.querySelector('a')?.getAttribute('href')).not.toContain('javascript:')
})

it('supports clearing a note and does not lose newer edits during a save', async () => {
  let finish!: () => void
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string) => {
          if (command === 'contacts.noteGet') return { prefs: {} }
          await new Promise<void>((resolve) => {
            finish = resolve
          })
          return { ok: true }
        },
      },
    },
  }
  await loadContactNote('alice@example.com')
  contactNotes$['alice@example.com'].draft.set('First')
  const saving = saveContactNote('alice@example.com')
  contactNotes$['alice@example.com'].draft.set('Newer edit')
  finish()
  await saving
  expect(contactNotes$['alice@example.com'].saved.peek()).toBe('First')
  expect(contactNotes$['alice@example.com'].draft.peek()).toBe('Newer edit')
  contactNotes$['alice@example.com'].draft.set('')
  const clearing = saveContactNote('alice@example.com')
  finish()
  await clearing
  expect(contactNotes$['alice@example.com'].saved.peek()).toBe('')
})

it('saves with Ctrl+Enter and cancels with Escape', async () => {
  const prefs: Record<string, string> = {}
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          if (command === 'contacts.noteGet') return { prefs }
          prefs[payload.key] = payload.value
          return { ok: true }
        },
      },
    },
  }
  const view = render(<ContactNotes addr="alice@example.com" />)
  await settle()
  fireEvent.click(view.getByRole('button', { name: 'Add a note…' }))
  fireEvent.change(view.getByRole('textbox'), { target: { value: 'Saved by keyboard' } })
  fireEvent.keyDown(view.getByRole('textbox'), { key: 'Enter', ctrlKey: true })
  await settle()
  expect(prefs['contact_note:alice@example.com']).toBe('Saved by keyboard')
  expect(view.queryByRole('textbox')).toBeNull()
  fireEvent.click(view.getByRole('button', { name: 'Edit' }))
  fireEvent.change(view.getByRole('textbox'), { target: { value: 'Discard me' } })
  fireEvent.keyDown(view.getByRole('textbox'), { key: 'Escape' })
  expect(view.queryByRole('textbox')).toBeNull()
  expect(contactNotes$['alice@example.com'].draft.peek()).toBe('Saved by keyboard')
})
