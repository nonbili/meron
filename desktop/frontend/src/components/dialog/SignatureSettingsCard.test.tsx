import { afterEach, beforeEach, describe, expect, it, mock } from 'bun:test'
import { act, cleanup, render } from '@testing-library/react'
import type { Editor } from '@tiptap/react'
import * as tiptapReact from '@tiptap/react'
import { accounts$ } from '../../states/accounts'
import { settings$ } from '../../states/settings'
import type { Account } from '../../types'

// The card's editors are real; this only captures the instance each one returns
// so a test can type into it the way the user would. The snapshot is taken
// before the module is replaced, or the delegation below would call itself.
const realTiptap = { ...tiptapReact }
let editors: Editor[] = []
mock.module('@tiptap/react', () => ({
  ...realTiptap,
  useEditor: (options: Parameters<typeof tiptapReact.useEditor>[0], deps?: unknown[]) => {
    const editor = realTiptap.useEditor(options, deps)
    if (editor && !editors.includes(editor)) editors.push(editor)
    return editor
  },
}))

const { AccountSignatureCard } = await import('./SignatureSettingsCard')

const account = (id: string, html: string, mode: 'global' | 'none' | 'custom' = 'custom'): Account => ({
  id,
  email: `${id}@example.com`,
  display_name: id,
  provider: 'custom',
  auth_type: 'password',
  imap_host: 'imap.example.com',
  imap_port: 993,
  smtp_host: 'smtp.example.com',
  smtp_port: 465,
  tls: true,
  signature: { mode, html },
})

type Call = { command: string; payload: any }

describe('AccountSignatureCard', () => {
  let calls: Call[] = []

  beforeEach(() => {
    calls = []
    editors = []
    settings$.signature.set('')
    // 'c' follows the app-wide signature: switching to it is what tells a
    // flush filed under the *current* mode apart from one filed under the mode
    // its own account had.
    accounts$.set([account('a', '<p>A</p>'), account('b', '<p>B</p>'), account('c', '', 'global')])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return {}
          },
        },
      },
    }
  })

  afterEach(cleanup)

  const signatureCalls = () => calls.filter((call) => call.command === 'account.setSignature')
  const editorHtml = () => editors[editors.length - 1]?.getHTML() ?? ''
  const type = async (html: string) => {
    await act(async () => {
      editors[editors.length - 1]?.commands.setContent(html)
    })
  }

  it('seeds a switched-to account with its own signature, not the previous one', async () => {
    const view = render(<AccountSignatureCard account={accounts$.peek()[0]} />)
    expect(editorHtml()).toBe('<p>A</p>')

    await act(async () => {
      view.rerender(<AccountSignatureCard account={accounts$.peek()[1]} />)
    })

    expect(editorHtml()).toBe('<p>B</p>')
    // Re-seeding is not an edit: nothing should have been written anywhere.
    expect(signatureCalls()).toEqual([])
  })

  it('files an edit still pending at the moment of the switch against the account it was typed in', async () => {
    const view = render(<AccountSignatureCard account={accounts$.peek()[0]} />)
    await type('<p>A, edited</p>')

    // The switch unmounts A's editor before its debounce has elapsed. The
    // account switched to follows the app-wide signature, so a flush that took
    // its mode from the card rather than from the editor's own account would
    // file A's text as 'global' and quietly turn A's custom signature off.
    await act(async () => {
      view.rerender(<AccountSignatureCard account={accounts$.peek()[2]} />)
    })

    expect(signatureCalls()).toHaveLength(1)
    expect(signatureCalls()[0].payload).toEqual({
      id: 'a',
      signature: { mode: 'custom', html: '<p>A, edited</p>' },
    })
  })

  it('keeps the mode the user just picked when a pending edit lands after it', async () => {
    render(<AccountSignatureCard account={accounts$.peek()[0]} />)
    await type('<p>A, edited</p>')

    // Choosing "None" unmounts the editor, which flushes the pending edit.
    const select = document.querySelector('select') as HTMLSelectElement
    await act(async () => {
      select.value = 'none'
      select.dispatchEvent(new Event('change', { bubbles: true }))
    })

    const written = signatureCalls()
    expect(written.map((call) => call.payload.id)).toEqual(['a', 'a'])
    // Whatever order they were queued in, the account is left opted out — with
    // the text preserved, so switching back to Custom restores it.
    expect(written[written.length - 1].payload.signature).toEqual({
      mode: 'none',
      html: '<p>A, edited</p>',
    })
  })

  it('goes back to the stored signature when the write is rejected', async () => {
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'account.setSignature') throw new Error('write failed')
      return {}
    }
    render(<AccountSignatureCard account={accounts$.peek()[0]} />)

    const select = document.querySelector('select') as HTMLSelectElement
    await act(async () => {
      select.value = 'none'
      select.dispatchEvent(new Event('change', { bubbles: true }))
      await new Promise((resolve) => setTimeout(resolve, 20))
    })

    // The rejected choice must not stay on screen: the account still has its
    // custom signature, and the card has to say so.
    expect((document.querySelector('select') as HTMLSelectElement).value).toBe('custom')
  })

  it('writes an edit to the account being shown when nothing else changed', async () => {
    render(<AccountSignatureCard account={accounts$.peek()[0]} />)
    await type('<p>A, edited</p>')

    // The debounce, rather than an unmount, carries this one.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 700))
    })

    expect(signatureCalls()).toHaveLength(1)
    expect(signatureCalls()[0].payload).toEqual({
      id: 'a',
      signature: { mode: 'custom', html: '<p>A, edited</p>' },
    })
  })
})
