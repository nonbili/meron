import { afterEach, beforeEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { SearchBox } from './SearchBox'

beforeEach(() => {
  ;(window as any).go = {
    main: {
      App: {
        Invoke: (command: string, payload: { query?: string }) =>
          Promise.resolve(
            command === 'search.parse'
              ? {
                  from: '',
                  to: '',
                  subject: '',
                  words: payload.query,
                  after: '',
                  before: '',
                  unread: false,
                  starred: false,
                }
              : { query: '' },
          ),
      },
    },
  }
})

afterEach(() => {
  cleanup()
  delete (window as any).go
})

// Closing the form unmounts its focused field, which fires no blur: the box
// takes focus back so whatever tracks its focus sees it leave later.
it('gives focus back to the search input when the options form closes', async () => {
  const focus: boolean[] = []
  render(
    <SearchBox
      value=""
      onChange={() => undefined}
      placeholder="Search"
      onFocusChange={(focused) => focus.push(focused)}
      options={{ onSearch: () => undefined }}
    />,
  )
  // Opening awaits search.parse. Flush that update and its effects so Escape's
  // handler is installed; autofocus happens before passive effects run.
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: 'Search options' }))
  })
  await waitFor(() => expect(document.activeElement?.closest('form')).toBeTruthy())
  fireEvent.keyDown(window, { key: 'Escape' })
  await waitFor(() => expect(document.querySelector('form')).toBeNull())
  expect(document.activeElement).toBe(screen.getByPlaceholderText('Search'))
  fireEvent.blur(screen.getByPlaceholderText('Search'))
  expect(focus.at(-1)).toBe(false)
})
