import { afterEach, expect, it, mock } from 'bun:test'
import { cleanup, fireEvent, render } from '@testing-library/react'
import { ColorSwatchPicker } from './ColorSwatchPicker'

afterEach(cleanup)

const checked = (container: HTMLElement) =>
  Array.from(container.querySelectorAll('[role="radio"][aria-checked="true"]')).map((el) =>
    el.getAttribute('aria-label'),
  )
const customInput = (container: HTMLElement) => container.querySelector<HTMLInputElement>('input[type="color"]')!

it('picks a preset in one click and offers none only when asked', () => {
  const onChange = mock((_color: string | null) => {})
  const { container, getByLabelText, queryByLabelText } = render(
    <ColorSwatchPicker value={null} onChange={onChange} label="Color" />,
  )
  expect(queryByLabelText('No color')).toBeNull()
  expect(checked(container)).toEqual([])
  fireEvent.click(getByLabelText('Blue'))
  expect(onChange).toHaveBeenCalledWith('#3b82f6')
})

it('marks the none swatch for no colour and the matching preset otherwise', () => {
  const props = { onChange: () => {}, label: 'Color', noneLabel: 'No color' }
  const { container, rerender } = render(<ColorSwatchPicker value={null} {...props} />)
  expect(checked(container)).toEqual(['No color'])
  rerender(<ColorSwatchPicker value="#3B82F6" {...props} />)
  expect(checked(container)).toEqual(['Blue'])
})

it('shows a colour outside the presets as the selected custom swatch', () => {
  const { container } = render(<ColorSwatchPicker value="#1d6fe0" onChange={() => {}} label="Color" />)
  expect(checked(container)).toEqual(['Custom color…'])
  expect(customInput(container).value).toBe('#1d6fe0')
})

it('previews while the system picker drags and commits only when it closes', () => {
  const onChange = mock((_color: string | null) => {})
  const onPreview = mock((_color: string) => {})
  const { container } = render(
    <ColorSwatchPicker value={null} onChange={onChange} onPreview={onPreview} label="Color" noneLabel="No color" />,
  )
  const input = customInput(container)
  fireEvent.input(input, { target: { value: '#123456' } })
  fireEvent.input(input, { target: { value: '#abcdef' } })
  expect(onPreview.mock.calls).toEqual([['#123456'], ['#abcdef']])
  expect(onChange).not.toHaveBeenCalled()
  expect(checked(container)).toEqual(['Custom color…'])

  fireEvent.change(input, { target: { value: '#abcdef' } })
  expect(onChange).toHaveBeenCalledWith('#abcdef')
})
