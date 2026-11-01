import { afterEach, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { useState } from 'react'
import { completeTag, removeTag, tagSuggestions, TagInput } from './TagInput'

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

it('takes one listed tag out, keeping what is being typed', () => {
  expect(removeTag('todo, work ', 0)).toBe('work ')
  expect(removeTag('todo work ', 1, 'ho')).toBe('todo ho')
})

function renderField(initial = '') {
  const seen = { value: initial }
  function Field() {
    const [value, setValue] = useState(initial)
    seen.value = value
    return <TagInput value={value} onChange={setValue} />
  }
  return { view: render(<Field />), seen }
}

const chipLabels = () =>
  Array.from(document.querySelectorAll('button[aria-label^="Remove"]')).map((chip) => chip.getAttribute('aria-label'))

it('shows finished tags as chips, and every tag once away from the field', async () => {
  ;(window as any).go = { main: { App: { Invoke: () => Promise.resolve({ tags: [] }) } } }
  const { view, seen } = renderField('work todo')
  const input = view.getByRole('textbox') as HTMLInputElement
  expect(chipLabels()).toEqual(['Remove work', 'Remove todo'])
  expect(input.value).toBe('')

  fireEvent.focus(input)
  expect(chipLabels()).toEqual(['Remove work', 'Remove todo'])
  fireEvent.change(input, { target: { value: 'ho' } })
  expect(chipLabels()).toEqual(['Remove work', 'Remove todo'])
  expect(input.value).toBe('ho')
  expect(seen.value).toBe('work todo ho')

  fireEvent.blur(input)
  expect(chipLabels()).toEqual(['Remove work', 'Remove todo', 'Remove ho'])
  expect(input.value).toBe('')
})

it('removes a chip by its button, or the last with Backspace on an empty input', async () => {
  ;(window as any).go = { main: { App: { Invoke: () => Promise.resolve({ tags: [] }) } } }
  const { view, seen } = renderField('work todo home ')
  const input = view.getByRole('textbox') as HTMLInputElement
  fireEvent.focus(input)
  fireEvent.click(view.getByLabelText('Remove todo'))
  expect(seen.value).toBe('work home ')

  fireEvent.change(input, { target: { value: 'x' } })
  expect(fireEvent.keyDown(input, { key: 'Backspace' })).toBe(true)
  expect(seen.value).toBe('work home x')
  fireEvent.change(input, { target: { value: '' } })
  expect(fireEvent.keyDown(input, { key: 'Backspace' })).toBe(false)
  expect(seen.value).toBe('work ')
})

it('fills in a picked tag, and Enter stays with the form until one is chosen', async () => {
  ;(window as any).go = { main: { App: { Invoke: () => Promise.resolve({ tags: known }) } } }
  const { view, seen } = renderField()
  const input = view.getByRole('textbox') as HTMLInputElement
  fireEvent.focus(input)
  await waitFor(() => expect(view.getByText('todo')).toBeTruthy())
  fireEvent.change(input, { target: { value: 'to' } })
  expect(fireEvent.keyDown(input, { key: 'Enter' })).toBe(true)
  fireEvent.keyDown(input, { key: 'ArrowDown' })
  expect(fireEvent.keyDown(input, { key: 'Enter' })).toBe(false)
  expect(seen.value).toBe('todo ')
  expect(input.value).toBe('')
  expect(chipLabels()).toEqual(['Remove todo'])
})
