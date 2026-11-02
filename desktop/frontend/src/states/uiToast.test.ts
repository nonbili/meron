import { afterEach, beforeEach, expect, it, jest } from 'bun:test'
import { dismissToast, runToastUndo, showToast, showUndoToast, ui$ } from './ui'

beforeEach(() => {
  dismissToast()
  jest.useFakeTimers()
})

afterEach(() => {
  dismissToast()
  jest.useRealTimers()
})

it('keeps errors visible until dismissed', () => {
  showToast('Copy failed: the message is too large.', 'error')
  jest.advanceTimersByTime(60_000)
  expect(ui$.toast.get()).toContain('Copy failed')
  dismissToast()
  expect(ui$.toast.get()).toBe('')
})

it('still dismisses success confirmations automatically', () => {
  showToast('Copied')
  jest.advanceTimersByTime(2200)
  expect(ui$.toast.get()).toBe('')
})

it('cancels an older timer even when the same message is shown again', () => {
  showToast('Server unavailable', 'error', 2200)
  jest.advanceTimersByTime(1000)
  showToast('Server unavailable', 'error')
  jest.advanceTimersByTime(60_000)
  expect(ui$.toast.get()).toBe('Server unavailable')
})

it('keeps the full undo window when a message is repeated', () => {
  let undone = false
  showToast('Moved')
  jest.advanceTimersByTime(1000)
  showUndoToast('Moved', () => {
    undone = true
  })
  jest.advanceTimersByTime(2200)
  expect(ui$.toast.get()).toBe('Moved')
  runToastUndo()
  expect(undone).toBe(true)
  expect(ui$.toast.get()).toBe('')
  expect(ui$.toastUndo.get()).toBeNull()
})

it('an undo toast replacing another runs its own undo', () => {
  const ran: string[] = []
  showUndoToast('First', () => ran.push('first'))
  showUndoToast('Second', () => ran.push('second'))

  runToastUndo()

  expect(ran).toEqual(['second'])
})
