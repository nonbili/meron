import { afterEach, beforeEach, expect, it, jest } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { DndContext } from '@dnd-kit/core'
import { SortableContext } from '@dnd-kit/sortable'

import { TaskRow } from './TaskRow'
import { useTaskDragSensors } from './taskDrag'
import type { Task } from '../../states/tasks'

const task: Task = {
  id: 'task',
  list_id: 'list',
  title: 'Follow up',
  notes: 'First line\nSecond line',
  due_at: 0,
  completed_at: 0,
  done: false,
  account: '',
  thread_id: 'thread',
  message_id: '',
}
beforeEach(() => jest.useFakeTimers())
afterEach(() => {
  cleanup()
  // dnd-kit retains its document click listener for 50ms after dragging.
  act(() => jest.advanceTimersByTime(60))
  jest.useRealTimers()
})

function fixture(expanded = false) {
  let starts = 0
  let opens = 0
  function Rows() {
    const sensors = useTaskDragSensors()
    return (
      <DndContext
        sensors={sensors}
        onDragStart={() => {
          starts++
        }}
      >
        <SortableContext items={[task.id]}>
          <TaskRow
            task={task}
            sortable
            expanded={expanded}
            onToggle={() => {}}
            onOpen={() => {
              opens++
            }}
            onDelete={() => {}}
            onOpenMessage={() => {}}
            renderEditor={(actions) => (
              <div>
                <input aria-label="Inline title" defaultValue={task.title} />
                {actions}
              </div>
            )}
          />
        </SortableContext>
      </DndContext>
    )
  }
  const view = render(<Rows />)
  const row = view.getByRole('checkbox').parentElement!.parentElement!
  return { view, row, starts: () => starts, opens: () => opens }
}

it('keeps the row out of the tab order and exposes its controls', () => {
  const { view, row } = fixture()
  expect(row.hasAttribute('role')).toBe(false)
  expect(row.hasAttribute('tabindex')).toBe(false)
  expect(row.hasAttribute('aria-roledescription')).toBe(false)
  expect(view.getByRole('button', { name: /Follow up/ }).tagName).toBe('BUTTON')
  expect(view.getByRole('checkbox', { name: task.title })).toBeTruthy()
})

it('drags from empty row space after the mouse threshold', () => {
  const f = fixture()
  fireEvent.mouseDown(f.row, { button: 0, clientX: 0, clientY: 0 })
  fireEvent.mouseMove(document, { clientX: 0, clientY: 5 })
  expect(f.starts()).toBe(0)
  fireEvent.mouseMove(document, { clientX: 0, clientY: 10 })
  expect(f.starts()).toBe(1)
  fireEvent.mouseUp(document)
})

it('opens on a title click and suppresses the click after dragging the title', () => {
  const f = fixture()
  const title = f.view.getByRole('button', { name: /Follow up/ })
  fireEvent.mouseDown(title, { button: 0, clientX: 0, clientY: 0 })
  fireEvent.mouseUp(document)
  fireEvent.click(title)
  expect(f.opens()).toBe(1)
  fireEvent.mouseDown(title, { button: 0, clientX: 0, clientY: 0 })
  fireEvent.mouseMove(document, { clientX: 0, clientY: 10 })
  fireEvent.mouseUp(document)
  fireEvent.click(title)
  expect(f.starts()).toBe(1)
  expect(f.opens()).toBe(1)
})

it('does not drag from checkboxes, actions, or inline fields', () => {
  const f = fixture(true)
  const controls = [f.view.getByRole('checkbox'), ...f.view.getAllByRole('button'), f.view.getByRole('textbox')]
  for (const control of controls) {
    fireEvent.mouseDown(control, { button: 0, clientX: 0, clientY: 0 })
    fireEvent.mouseMove(document, { clientX: 0, clientY: 10 })
    fireEvent.mouseUp(document)
    fireEvent.touchStart(control, { touches: [{ clientX: 0, clientY: 0 }] })
    act(() => jest.advanceTimersByTime(250))
    expect(f.starts()).toBe(0)
    fireEvent.touchEnd(control, { touches: [] })
  }
  expect(f.starts()).toBe(0)
})

it('allows control presses to reach document and window outside-click handlers', () => {
  const f = fixture(true)
  const controls = [f.view.getByRole('checkbox'), ...f.view.getAllByRole('button'), f.view.getByRole('textbox')]
  let documentPresses = 0
  let windowPresses = 0
  const onDocumentPress = () => {
    documentPresses++
  }
  const onWindowPress = () => {
    windowPresses++
  }
  document.addEventListener('mousedown', onDocumentPress)
  window.addEventListener('mousedown', onWindowPress)
  try {
    for (const control of controls) fireEvent.mouseDown(control, { button: 0 })
    expect(documentPresses).toBe(controls.length)
    expect(windowPresses).toBe(controls.length)
    expect(f.starts()).toBe(0)
  } finally {
    document.removeEventListener('mousedown', onDocumentPress)
    window.removeEventListener('mousedown', onWindowPress)
  }
})

it('allows a touch swipe without dragging and starts dragging after a hold', () => {
  const f = fixture()
  expect(f.row.classList.contains('touch-none')).toBe(false)
  fireEvent.touchStart(f.row, { touches: [{ clientX: 0, clientY: 0 }] })
  expect(fireEvent.touchMove(f.row, { touches: [{ clientX: 0, clientY: 10 }] })).toBe(true)
  act(() => jest.advanceTimersByTime(250))
  expect(f.starts()).toBe(0)
  fireEvent.touchEnd(f.row, { touches: [] })
  fireEvent.touchStart(f.row, { touches: [{ clientX: 0, clientY: 0 }] })
  act(() => jest.advanceTimersByTime(249))
  expect(f.starts()).toBe(0)
  act(() => jest.advanceTimersByTime(1))
  expect(f.starts()).toBe(1)
  expect(fireEvent.touchMove(f.row, { touches: [{ clientX: 0, clientY: 10 }] })).toBe(false)
  fireEvent.touchEnd(f.row, { touches: [] })
})

it('opens a link in the notes without opening the editor', () => {
  const open = jest.spyOn(window, 'open').mockImplementation(() => null)
  const linked = { ...task, notes: 'Join: https://meet.example.com/abc-defg.' }
  const view = render(
    <TaskRow
      task={linked}
      sortable={false}
      expanded={false}
      onToggle={() => {}}
      onOpen={() => {
        throw new Error('opened the editor')
      }}
      onDelete={() => {}}
    />,
  )
  const link = view.getByRole('link', { name: 'https://meet.example.com/abc-defg' })
  fireEvent.click(link)
  expect(open).toHaveBeenCalledWith('https://meet.example.com/abc-defg', '_blank', 'noreferrer')
  open.mockRestore()
})

it('opens the editor from the notes text', () => {
  const f = fixture()
  fireEvent.click(f.view.getByText(/First line/))
  expect(f.opens()).toBe(1)
})

it('offers to copy a link in the notes from its context menu', async () => {
  const writes: string[] = []
  const clipboard = navigator.clipboard
  Object.defineProperty(navigator, 'clipboard', {
    value: { writeText: async (text: string) => void writes.push(text) },
    configurable: true,
  })
  const linked = { ...task, notes: 'Join: https://meet.example.com/abc-defg' }
  let opens = 0
  const view = render(
    <TaskRow
      task={linked}
      sortable={false}
      expanded={false}
      onToggle={() => {}}
      onOpen={() => opens++}
      onDelete={() => {}}
    />,
  )
  fireEvent.contextMenu(view.getByRole('link', { name: 'https://meet.example.com/abc-defg' }))
  fireEvent.click(view.getByRole('button', { name: 'Copy link address' }))
  await act(async () => {})
  expect(writes).toEqual(['https://meet.example.com/abc-defg'])
  expect(opens).toBe(0)
  expect(view.queryByRole('button', { name: 'Copy link address' })).toBeNull()
  Object.defineProperty(navigator, 'clipboard', { value: clipboard, configurable: true })
})
