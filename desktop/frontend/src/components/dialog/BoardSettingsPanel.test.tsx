import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
// Load the stores before the panel, as the app does; importing the panel first
// initialises states/quickReply ahead of the mail$ store it subscribes to.
import '../../states/accounts'
import { settings$, type KanbanBoard } from '../../states/settings'
import { BoardPanel } from './BoardSettingsPanel'

const board = (avatarUrl?: string): KanbanBoard =>
  ({ id: 'b1', name: 'Status', columns: [], ...(avatarUrl ? { avatarUrl } : {}) }) as KanbanBoard

describe('BoardPanel image', () => {
  beforeEach(() => {
    ;(window as any).go = { main: { App: { Invoke: async () => ({}) } } }
  })

  afterEach(cleanup)

  it('removes a custom board image from the settings header', async () => {
    settings$.kanbanBoards.set([board('/media/avatars/b1/pic.png')])
    const view = render(<BoardPanel board={settings$.kanbanBoards.peek()[0]} />)

    await act(async () => {
      fireEvent.click(view.getByRole('button', { name: 'Remove board image' }))
    })

    expect(settings$.kanbanBoards.peek()[0].avatarUrl).toBeUndefined()
  })

  it('offers no remove button while the board has no image', () => {
    settings$.kanbanBoards.set([board()])
    const view = render(<BoardPanel board={settings$.kanbanBoards.peek()[0]} />)
    expect(view.queryByRole('button', { name: 'Remove board image' })).toBeNull()
  })
})

describe('BoardPanel unread badge', () => {
  afterEach(cleanup)

  it('turns the rail unread badge on and off for this board', () => {
    settings$.kanbanBoards.set([board()])
    const view = render(<BoardPanel board={settings$.kanbanBoards.peek()[0]} />)
    const toggle = view.getByRole('switch', { name: 'Show unread badge' })
    expect(toggle.getAttribute('aria-checked')).toBe('false')

    fireEvent.click(toggle)
    expect(settings$.kanbanBoards.peek()[0].showUnreadBadge).toBe(true)

    view.rerender(<BoardPanel board={settings$.kanbanBoards.peek()[0]} />)
    fireEvent.click(view.getByRole('switch', { name: 'Show unread badge' }))
    expect(settings$.kanbanBoards.peek()[0]).not.toHaveProperty('showUnreadBadge')
  })
})
