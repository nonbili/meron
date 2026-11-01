import { afterEach, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { useState } from 'react'
import { completeTag, tagSuggestions, TagInput } from './TagInput'

const known = [
  { tag: 'work', count: 1 },
  { tag: 'homework', count: 5 },
  { tag: 'todo', count: 2 },
]

afterEach(() => {
  cleanup()
  delete (window as any).go
})

it('suggests by what is typed, most used first, without tags already listed', () => {
  expect(tagSuggestions(known, 'wor')).toEqual(['homework', 'work'])
  expect(tagSuggestions(known, 'work, ')).toEqual(['homework', 'todo'])
  expect(tagSuggestions(known, '')).toEqual(['homework', 'todo', 'work'])
})

it('completes only the tag being typed', () => {
  expect(completeTag('wor', 'work')).toBe('work ')
  expect(completeTag('todo, wo', 'work')).toBe('todo, work ')
})

it('fills in a picked tag, and Enter stays with the form until one is chosen', async () => {
  ;(window as any).go = { main: { App: { Invoke: () => Promise.resolve({ tags: known }) } } }
  function Field() {
    const [value, setValue] = useState('')
    return <TagInput value={value} onChange={setValue} className="" />
  }
  const view = render(<Field />)
  const input = view.getByRole('textbox') as HTMLInputElement
  fireEvent.focus(input)
  await waitFor(() => expect(view.getByText('todo')).toBeTruthy())
  fireEvent.change(input, { target: { value: 'to' } })
  expect(fireEvent.keyDown(input, { key: 'Enter' })).toBe(true)
  fireEvent.keyDown(input, { key: 'ArrowDown' })
  expect(fireEvent.keyDown(input, { key: 'Enter' })).toBe(false)
  expect(input.value).toBe('todo ')
})
