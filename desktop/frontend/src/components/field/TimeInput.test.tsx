import { afterEach, expect, it, mock } from 'bun:test'
import { cleanup, fireEvent, render } from '@testing-library/react'
import { TimeInput } from './TimeInput'

afterEach(cleanup)

const list = () => document.body.querySelector('[role="listbox"]')
const options = () => Array.from(document.body.querySelectorAll<HTMLElement>('[role="option"]'))

function setup(value = '09:00') {
  const onChange = mock((_value: string) => {})
  const view = render(<TimeInput aria-label="Starts" value={value} onChange={onChange} />)
  return { ...view, onChange, input: view.getByLabelText('Starts') }
}

it('opens half-hour slots on click, marking the current time', () => {
  const { input } = setup()
  expect(list()).toBeNull()
  fireEvent.click(input)
  expect(options()).toHaveLength(48)
  const selected = options().filter((option) => option.getAttribute('aria-selected') === 'true')
  expect(selected).toEqual([options()[18]])
})

it('picks a slot and closes', () => {
  const { input, onChange } = setup()
  fireEvent.click(input)
  fireEvent.click(options()[21])
  expect(onChange).toHaveBeenCalledWith('10:30')
  expect(list()).toBeNull()
})

it('still takes a typed time', () => {
  const { input, onChange } = setup()
  fireEvent.change(input, { target: { value: '09:17' } })
  expect(onChange).toHaveBeenCalledWith('09:17')
})

it('closes on typing, Escape, and a press outside', () => {
  const { input } = setup()
  fireEvent.click(input)
  fireEvent.keyDown(input, { key: '1' })
  expect(list()).toBeNull()

  fireEvent.click(input)
  fireEvent.keyDown(input, { key: 'Escape' })
  expect(list()).toBeNull()

  fireEvent.click(input)
  fireEvent.pointerDown(document.body)
  expect(list()).toBeNull()
})

it('keeps focus in the input when a slot is pressed', () => {
  const { input } = setup()
  fireEvent.click(input)
  expect(fireEvent.mouseDown(options()[0])).toBe(false)
})

it('opens on Alt+Down into the list, which the arrows move and Enter picks from', () => {
  const { input, onChange } = setup()
  fireEvent.keyDown(input, { key: 'ArrowDown' })
  expect(list()).toBeNull()
  fireEvent.keyDown(input, { key: 'ArrowDown', altKey: true })
  const slots = list() as HTMLElement
  expect(input.getAttribute('aria-expanded')).toBe('true')
  expect(document.activeElement).toBe(slots)
  expect(slots.getAttribute('aria-activedescendant')).toBe(options()[18].id)
  expect(fireEvent.keyDown(slots, { key: 'ArrowDown' })).toBe(false)
  fireEvent.keyDown(slots, { key: 'ArrowDown' })
  fireEvent.keyDown(slots, { key: 'ArrowUp' })
  expect(slots.getAttribute('aria-activedescendant')).toBe(options()[19].id)
  fireEvent.keyDown(slots, { key: 'Enter' })
  expect(onChange).toHaveBeenCalledWith('09:30')
  expect(list()).toBeNull()
  expect(document.activeElement).toBe(input)
})

it('returns focus to the input when Escape or Tab leaves the list', () => {
  const { input } = setup()
  fireEvent.keyDown(input, { key: 'ArrowDown', altKey: true })
  fireEvent.keyDown(document.activeElement!, { key: 'Escape' })
  expect(list()).toBeNull()
  expect(document.activeElement).toBe(input)

  fireEvent.keyDown(input, { key: 'ArrowDown', altKey: true })
  expect(fireEvent.keyDown(document.activeElement!, { key: 'Tab' })).toBe(false)
  expect(list()).toBeNull()
  expect(document.activeElement).toBe(input)
})

it('keeps bare keys pressed in the list from the app shortcuts, but not chords', () => {
  const { input } = setup()
  const seen = mock((_event: KeyboardEvent) => {})
  window.addEventListener('keydown', seen)
  try {
    fireEvent.keyDown(input, { key: 'ArrowDown', altKey: true })
    fireEvent.keyDown(document.activeElement!, { key: 'ArrowDown' })
    fireEvent.keyDown(document.activeElement!, { key: 'e' })
    expect(seen).toHaveBeenCalledTimes(1)
    fireEvent.keyDown(document.activeElement!, { key: 'q', ctrlKey: true })
    expect(seen).toHaveBeenCalledTimes(2)
  } finally {
    window.removeEventListener('keydown', seen)
  }
})

it('renders the list inside the modal that holds the input', () => {
  const view = render(
    <div data-dialog-surface="" aria-modal="true">
      <TimeInput aria-label="Starts" value="09:00" onChange={() => {}} />
    </div>,
  )
  fireEvent.click(view.getByLabelText('Starts'))
  expect(list()?.parentElement).toBe(view.container.querySelector('[data-dialog-surface]'))
})

it('only closes the list on Enter in the input, leaving Enter to the form once typing closes it', () => {
  const { input, onChange } = setup('09:17')
  fireEvent.click(input)
  expect(fireEvent.keyDown(input, { key: 'Enter' })).toBe(false)
  expect(onChange).not.toHaveBeenCalled()
  expect(list()).toBeNull()

  fireEvent.click(input)
  fireEvent.keyDown(input, { key: '1' })
  expect(fireEvent.keyDown(input, { key: 'Enter' })).toBe(true)
})
