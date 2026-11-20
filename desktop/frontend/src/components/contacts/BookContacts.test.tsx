import { afterEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { BookContacts } from './BookContacts'

afterEach(cleanup)

it('requires confirmation and sends the loaded contact version when deleting', async () => {
  const calls: Array<[string, Record<string, unknown>]> = []
  let deleted = false
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (method: string, params: Record<string, unknown>) => {
          calls.push([method, params])
          if (method === 'carddav.deleteContact') {
            deleted = true
            return {}
          }
          return {
            contacts: deleted ? [] : [{ id: 'cora.vcf', version: '"v1"', name: 'Cora Fixture', emails: [] }],
            more: false,
          }
        },
      },
    },
  }
  const view = render(<BookContacts bookId="book-id" onChange={() => {}} disabled={false} />)
  fireEvent.click(await view.findByRole('button', { name: 'Cora Fixture' }))
  fireEvent.click(view.getByRole('button', { name: 'Delete contact' }))
  expect(deleted).toBe(false)
  await act(async () => fireEvent.click(view.getByRole('button', { name: 'Confirm delete' })))
  expect(calls).toContainEqual([
    'carddav.deleteContact',
    { book_id: 'book-id', contact_id: 'cora.vcf', version: '"v1"' },
  ])
  expect(view.queryByRole('button', { name: 'Cora Fixture' })).toBeNull()
})
