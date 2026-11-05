import { afterEach, beforeEach, expect, it } from 'bun:test'
import { useState } from 'react'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { RecipientInput } from './RecipientInput'

function Field({ onTab }: { onTab: () => void }) {
  const [value, setValue] = useState('')
  return <RecipientInput value={value} onChange={setValue} accountId="a" onTab={onTab} />
}

beforeEach(() => {
  ;(window as any).go = {
    main: { App: { Invoke: async () => ({ contacts: [{ name: '', addr: 'alice@example.com' }] }) } },
  }
})
afterEach(cleanup)

it('does not normalize text during IME composition', () => {
  const view = render(<Field onTab={() => {}} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  fireEvent.change(input, { target: { value: 'a@x.com, ' } })
  fireEvent.change(input, { target: { value: 'a@x.com,' } })
  const event = new Event('change', { bubbles: true })
  Object.defineProperty(event, 'isComposing', { value: true })
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!
  setter.call(input, 'a@x.com,や')
  fireEvent(input, event)
  expect(input.value).toBe('a@x.com,や')
})

it('allows backspacing through the trailing separator', () => {
  const view = render(<Field onTab={() => {}} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  fireEvent.change(input, { target: { value: 'a@x.com, ' } })
  fireEvent.change(input, { target: { value: 'a@x.com,' } })
  expect(input.value).toBe('a@x.com,')
  fireEvent.change(input, { target: { value: 'a@x.com' } })
  expect(input.value).toBe('a@x.com')
})

it('leaves mid-field edits and their caret intact', () => {
  const view = render(<Field onTab={() => {}} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  fireEvent.change(input, { target: { value: 'a@x.com, c@z.com' } })
  fireEvent.change(input, { target: { value: 'a@x.com, b@y.com c@z.com', selectionStart: 17, selectionEnd: 17 } })
  expect(input.value).toBe('a@x.com, b@y.com c@z.com')
  expect(input.selectionStart).toBe(17)
})

it('pastes an Excel rectangle before the input strips its separators', () => {
  const view = render(<Field onTab={() => {}} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  fireEvent.paste(input, { clipboardData: { getData: () => 'a@x.com\tb@y.com\r\nc@z.com' } })
  expect(input.value).toBe('a@x.com, b@y.com, c@z.com')
})

it('pastes at the selection without losing surrounding recipients', () => {
  const view = render(<Field onTab={() => {}} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  fireEvent.change(input, { target: { value: 'a@x.com, replace, d@z.com' } })
  input.setSelectionRange(9, 16)
  fireEvent.paste(input, { clipboardData: { getData: () => 'b@y.com\tc@z.com' } })
  expect(input.value).toBe('a@x.com, b@y.com, c@z.com, d@z.com')
})

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 160))
  })
}
async function showSuggestions(input: HTMLElement) {
  await act(async () => {
    input.focus()
  })
  fireEvent.change(input, { target: { value: 'al' } })
  await settle()
}

it('accepts with Tab, stays closed, then advances on the next Tab', async () => {
  let advances = 0
  const view = render(<Field onTab={() => advances++} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  await showSuggestions(input)
  expect(view.queryByRole('list')).not.toBeNull()
  fireEvent.keyDown(input, { key: 'Tab' })
  expect(input.value).toBe('alice@example.com, ')
  await settle()
  expect(view.queryByRole('list')).toBeNull()
  fireEvent.keyDown(input, { key: 'Tab' })
  expect(advances).toBe(1)
  expect(input.value).toBe('alice@example.com, ')
  fireEvent.change(input, { target: { value: 'alice@example.com, al' } })
  await settle()
  expect(view.queryByRole('list')).not.toBeNull()
})

it('leaves Shift+Tab to native navigation when suggestions are open', async () => {
  let advances = 0
  const view = render(<Field onTab={() => advances++} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  await showSuggestions(input)
  expect(view.queryByRole('list')).not.toBeNull()
  expect(fireEvent.keyDown(input, { key: 'Tab', shiftKey: true })).toBe(true)
  expect(input.value).toBe('al')
  expect(advances).toBe(0)
  expect(view.queryByRole('list')).toBeNull()
})

it('suggests nothing until something is typed', async () => {
  const view = render(<Field onTab={() => {}} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  await act(async () => {
    input.focus()
  })
  await settle()
  expect(view.queryByRole('list')).toBeNull()
  fireEvent.change(input, { target: { value: 'a' } })
  await settle()
  expect(view.queryByRole('list')).not.toBeNull()
})

it('reopens suggestions when focus returns to a token being typed', async () => {
  const view = render(<Field onTab={() => {}} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  await showSuggestions(input)
  expect(view.queryByRole('list')).not.toBeNull()
  await act(async () => {
    input.blur()
  })
  await settle()
  expect(view.queryByRole('list')).toBeNull()
  await act(async () => {
    input.focus()
  })
  expect(view.queryByRole('list')).not.toBeNull()
})

it('never reopens suggestions an edit has overtaken', async () => {
  let releaseBob!: () => void
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (_command: string, payload: { query: string }) => {
          if (payload.query.startsWith('bo')) {
            await new Promise<void>((resolve) => {
              releaseBob = resolve
            })
            return { contacts: [{ name: '', addr: 'bob@example.com' }] }
          }
          return { contacts: [{ name: '', addr: 'alice@example.com' }] }
        },
      },
    },
  }
  const view = render(<Field onTab={() => {}} />)
  const input = view.getByRole('textbox') as HTMLInputElement
  await showSuggestions(input)
  expect(view.getByRole('list').textContent).toContain('alice@example.com')

  // Replace the token, then leave before the new lookup returns.
  fireEvent.change(input, { target: { value: 'bo' } })
  await act(async () => {
    input.blur()
  })
  await settle()
  await act(async () => {
    input.focus()
  })
  expect(view.queryByRole('list')).toBeNull()

  // Once the lookup for the current token lands, focus brings that back.
  await act(async () => {
    input.blur()
  })
  await act(async () => {
    releaseBob()
  })
  await settle()
  await act(async () => {
    input.focus()
  })
  expect(view.getByRole('list').textContent).toContain('bob@example.com')
})
