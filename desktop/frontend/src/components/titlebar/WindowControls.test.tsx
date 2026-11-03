import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { windowChrome$ } from '../../lib/windowChrome'
import { kanban$ } from '../../states/kanban'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import { TitleBar } from './TitleBar'
import { WindowControls } from './WindowControls'

const original = (window as any).go
let calls: string[] = []

beforeEach(() => {
  calls = []
  ;(window as any).go = { main: { App: { Invoke: async (command: string) => void calls.push(command) } } }
  windowChrome$.set({
    supported: true,
    integrated: true,
    wanted: true,
    platform: 'linux',
    layout: { start: [], end: ['minimize', 'maximize', 'close'] },
    doubleClick: 'toggle-maximize',
    maximised: false,
  })
  ui$.query.set('')
  kanban$.activeBoardId.set('')
  kanban$.searchQuery.set('')
})

afterEach(() => {
  cleanup()
  settings$.tasksEnabled.set(true)
  ;(window as any).go = original
})

const labels = (buttons: HTMLElement[]) => buttons.map((button) => button.getAttribute('aria-label'))

describe('WindowControls', () => {
  it("draws the side's buttons in the desktop's layout, and runs each command", () => {
    render(<WindowControls side="end" />)
    const buttons = screen.getAllByRole('button')
    expect(labels(buttons)).toEqual(['Minimize', 'Maximize', 'Close'])
    buttons.forEach((button) => fireEvent.click(button))
    expect(calls).toEqual(['window.minimise', 'window.toggleMaximise', 'window.close'])
  })

  it('draws nothing for a side the layout leaves empty', () => {
    render(<WindowControls side="start" />)
    expect(screen.queryAllByRole('button')).toHaveLength(0)
  })

  it('draws nothing with the system title bar', () => {
    windowChrome$.integrated.set(false)
    render(<WindowControls side="end" />)
    expect(screen.queryAllByRole('button')).toHaveLength(0)
  })

  it('offers restore while maximised', () => {
    windowChrome$.maximised.set(true)
    render(<WindowControls side="end" />)
    expect(screen.getByRole('button', { name: 'Restore' })).toBeTruthy()
  })
})

describe('WindowControls on Windows', () => {
  it('draws the caption buttons, the close one marked for its red hover', () => {
    windowChrome$.platform.set('windows')
    render(<WindowControls side="end" />)
    const buttons = screen.getAllByRole('button')
    expect(labels(buttons)).toEqual(['Minimize', 'Maximize', 'Close'])
    expect(buttons.map((button) => button.className)).toEqual([
      'caption-button',
      'caption-button',
      'caption-button caption-button-close',
    ])
    buttons.forEach((button) => fireEvent.click(button))
    expect(calls).toEqual(['window.minimise', 'window.toggleMaximise', 'window.close'])
  })

  it('shows the restore glyph while maximised', () => {
    windowChrome$.platform.set('windows')
    windowChrome$.maximised.set(true)
    render(<WindowControls side="end" />)
    expect(screen.getByRole('button', { name: 'Restore' }).textContent).toBe('\uE923')
  })
})

describe('TitleBar', () => {
  it('holds the app buttons between the two sides of window controls', () => {
    windowChrome$.layout.set({ start: ['close'], end: ['minimize'] })
    settings$.tasksEnabled.set(false)
    settings$.calendarEnabled.set(false)
    render(<TitleBar />)
    expect(labels(screen.getAllByRole('button'))).toEqual(['Close', 'Search options', 'Contacts', 'More', 'Minimize'])
    act(() => settings$.tasksEnabled.set(true))
    expect(labels(screen.getAllByRole('button'))).toEqual([
      'Close',
      'Search options',
      'Contacts',
      'Tasks',
      'More',
      'Minimize',
    ])
    act(() => settings$.calendarEnabled.set(true))
    expect(labels(screen.getAllByRole('button'))).toEqual([
      'Close',
      'Search options',
      'Contacts',
      'Calendar panel',
      'Tasks',
      'More',
      'Minimize',
    ])
  })

  it("holds the thread list's search, or the kanban board's", () => {
    render(<TitleBar />)
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'invoice' } })
    expect(ui$.query.get()).toBe('invoice')
    fireEvent.click(screen.getByRole('button', { name: 'Clear search' }))
    expect(ui$.query.get()).toBe('')
    act(() => kanban$.activeBoardId.set('board-1'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'invoice' } })
    expect(kanban$.searchQuery.get()).toBe('invoice')
    expect(ui$.query.get()).toBe('')
  })

  it('leaves no search box on the setup screen', () => {
    render(<TitleBar tools={false} />)
    expect(screen.queryByRole('textbox')).toBeNull()
  })

  it('opens quick settings at the cursor when the title bar is right-clicked', () => {
    const { container } = render(<TitleBar />)
    const titleBar = container.querySelector('[data-titlebar]')!
    expect(fireEvent.contextMenu(titleBar, { clientX: 120, clientY: 20 })).toBe(false)
    const settings = screen.getByRole('button', { name: /Settings/ })
    const menu = settings.parentElement!
    expect(menu.style.left).toBe('120px')
    expect(menu.style.top).toBe('24px')
    expect(screen.getByRole('button', { name: 'Add kanban board' })).toBeTruthy()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByRole('button', { name: /Settings/ })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'More' }))
    expect(screen.getByRole('button', { name: /Settings/ })).toBeTruthy()
  })

  it('leaves the app buttons out on the setup screen', () => {
    render(<TitleBar tools={false} />)
    expect(labels(screen.getAllByRole('button'))).toEqual(['Minimize', 'Maximize', 'Close'])
  })

  it('is not drawn with the system title bar (off macOS)', () => {
    const { container } = render(<TitleBar />)
    expect(container.querySelector('[data-titlebar]')).not.toBeNull()
    act(() => windowChrome$.integrated.set(false))
    expect(container.querySelector('[data-titlebar]')).toBeNull()
  })
})
