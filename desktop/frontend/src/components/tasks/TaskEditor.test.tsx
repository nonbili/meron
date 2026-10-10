import { afterEach, beforeEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { useValue } from '@legendapp/state/react'

import { TaskEditor } from './TaskEditor'
import { tasks$, type Task } from '../../states/tasks'
import { ui$ } from '../../states/ui'

const task: Task = {
  id: 'task',
  list_id: 'list',
  title: 'Follow up',
  notes: 'First line\nSecond line',
  due_at: 0,
  completed_at: 0,
  done: false,
  account: '',
  thread_id: '',
  message_id: '',
}
let updates: Record<string, unknown>[]
let previousGo: PropertyDescriptor | undefined
let previousList: string
let previousTasks: ReturnType<typeof tasks$.peek>

beforeEach(() => {
  updates = []
  previousGo = Object.getOwnPropertyDescriptor(window, 'go')
  previousList = ui$.activeTaskList.peek()
  previousTasks = { ...tasks$.peek() }
  ui$.activeTaskList.set(task.list_id)
  tasks$.editingId.set(task.id)
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: Record<string, unknown>) => {
          if (command === 'tasks.update') updates.push(payload)
          if (command === 'tasks.items') return { tasks: [task] }
          return {}
        },
      },
    },
  }
})
afterEach(async () => {
  await act(async () => {})
  cleanup()
  if (previousGo) Object.defineProperty(window, 'go', previousGo)
  else delete (window as any).go
  ui$.activeTaskList.set(previousList)
  tasks$.set(previousTasks)
})

function EditableEditor() {
  const editingId = useValue(tasks$.editingId)
  return editingId === task.id ? <TaskEditor task={task} lists={[]} /> : null
}

it('shows title and multiline notes in the inline fields', () => {
  const view = render(<EditableEditor />)
  expect(view.getAllByRole('textbox')).toHaveLength(2)
  expect((view.getAllByRole('textbox')[0] as HTMLInputElement).value).toBe(task.title)
  expect((view.getAllByRole('textbox')[1] as HTMLTextAreaElement).value).toBe(task.notes)
})

it('saves focused notes and closes on the first Done click', async () => {
  const view = render(<EditableEditor />)
  const notes = view.getAllByRole('textbox')[1] as HTMLTextAreaElement
  notes.focus()
  fireEvent.change(notes, { target: { value: 'Updated\nnotes' } })
  const done = view.getByRole('button', { name: 'Done' })
  // Assert cancellation of the event that performs native focus transfer.
  // happy-dom does not implement that browser default action.
  expect(fireEvent.mouseDown(done)).toBe(false)
  expect(document.activeElement).toBe(notes)
  fireEvent.click(done)
  expect(view.queryAllByRole('textbox')).toHaveLength(0)
  await waitFor(() => expect(updates).toEqual([{ task_id: task.id, notes: 'Updated\nnotes' }]))
})

it('saves the focused title when Escape closes the row', async () => {
  const view = render(<EditableEditor />)
  const title = view.getAllByRole('textbox')[0]
  fireEvent.change(title, { target: { value: 'New title' } })
  fireEvent.keyDown(title, { key: 'Escape' })
  expect(view.queryAllByRole('textbox')).toHaveLength(0)
  await waitFor(() => expect(updates).toEqual([{ task_id: task.id, title: 'New title' }]))
})

it('saves the focused title and closes on a click outside the row', async () => {
  const view = render(
    <>
      <div data-task-row>
        <span>row padding</span>
        <EditableEditor />
      </div>
      <p>elsewhere</p>
    </>,
  )
  const title = view.getAllByRole('textbox')[0]
  fireEvent.change(title, { target: { value: 'New title' } })
  fireEvent.click(view.getByText('row padding'))
  expect(view.queryAllByRole('textbox')).toHaveLength(2)
  fireEvent.click(view.getByText('elsewhere'))
  expect(view.queryAllByRole('textbox')).toHaveLength(0)
  await waitFor(() => expect(updates).toEqual([{ task_id: task.id, title: 'New title' }]))
})

it('closes when focus moves outside the row, but waits for the click during a press', () => {
  const view = render(
    <>
      <EditableEditor />
      <button type="button">elsewhere</button>
    </>,
  )
  const outside = view.getByRole('button', { name: 'elsewhere' })
  fireEvent.mouseDown(outside)
  fireEvent.focusIn(outside)
  expect(view.queryAllByRole('textbox')).toHaveLength(2)
  fireEvent.mouseUp(outside)
  fireEvent.focusIn(view.getAllByRole('textbox')[1])
  expect(view.queryAllByRole('textbox')).toHaveLength(2)
  act(() => outside.focus())
  expect(view.queryAllByRole('textbox')).toHaveLength(0)
  expect(document.activeElement).toBe(outside)
})

it('keeps one resize observer while typing and remeasures when width changes', () => {
  const originalObserver = globalThis.ResizeObserver
  let observers = 0
  let disconnects = 0
  let notifyResize = () => {}
  globalThis.ResizeObserver = class {
    constructor(callback: () => void) {
      observers++
      notifyResize = callback
    }
    observe() {}
    disconnect() {
      disconnects++
    }
  } as unknown as typeof ResizeObserver
  try {
    const view = render(<EditableEditor />)
    const notes = view.getAllByRole('textbox')[1] as HTMLTextAreaElement
    let width = 200
    let height = 32
    Object.defineProperty(notes, 'clientWidth', { get: () => width, configurable: true })
    Object.defineProperty(notes, 'scrollHeight', { get: () => height, configurable: true })
    fireEvent.change(notes, { target: { value: 'More notes' } })
    fireEvent.change(notes, { target: { value: 'More notes still' } })
    expect(observers).toBe(1)
    expect(disconnects).toBe(0)
    act(notifyResize)
    expect(notes.style.height).toBe('32px')
    width = 100
    height = 64
    act(notifyResize)
    expect(notes.style.height).toBe('64px')
    view.unmount()
    expect(disconnects).toBe(1)
  } finally {
    cleanup()
    globalThis.ResizeObserver = originalObserver
  }
})
