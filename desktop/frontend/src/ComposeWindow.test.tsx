import { afterEach, beforeEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'

// Set the webview route before loading state modules: storage selection also
// happens before React mounts in the packaged app.
window.history.replaceState({}, '', '/?compose=standalone')
localStorage.setItem('meron-compose-tabs', '[{"id":"inbox-buffer"}]')
const { ComposeWindow } = await import('./ComposeWindow')
const { compose$ } = await import('./states/composeState')
const { updateComposeDraft, openComposeTab } = await import('./states/compose')
const { accounts$ } = await import('./states/accounts')
const { getComposeSession } = await import('./states/composeSessions')
const { dismissToast } = await import('./states/ui')

type Call = { command: string; payload: any }
let calls: Call[]
let handlers: Map<string, () => void>
let save: () => Promise<void>
let send: () => Promise<void>
let claim: boolean
let readAccounts: () => Promise<void>

beforeEach(() => {
  calls = []
  handlers = new Map()
  save = async () => {}
  send = async () => {}
  claim = true
  readAccounts = async () => {}
  compose$.tabs.set([])
  dismissToast()
  ;(window as any).runtime = {
    EventsOn: (name: string, callback: () => void) => {
      handlers.set(name, callback)
      return () => handlers.delete(name)
    },
  }
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push({ command, payload })
          switch (command) {
            case 'account.list':
              await readAccounts()
              return { accounts: [{ id: 'a', email: 'a@example.com', provider: 'custom', conversation_html: false }] }
            case 'app.prefsGet':
              return { prefs: {} }
            case 'composer.claimDraft':
              return claim
            case 'mail.allocateIdentity':
              return { message_id: 'draft@example.com' }
            case 'mail.saveDraft':
              await save()
              return {}
            case 'mail.send':
              await send()
              return {}
            default:
              return {}
          }
        },
      },
    },
  }
})
afterEach(() => cleanup())

async function mount() {
  const view = render(
    <StrictMode>
      <ComposeWindow />
    </StrictMode>,
  )
  await waitFor(() => expect(view.getByText('Discard')).toBeTruthy())
  const id = compose$.tabs.peek()[0]!.id
  act(() => updateComposeDraft(id, { to: 'recipient@example.com', subject: 'Hello', text: 'Latest text' }))
  return { view, id }
}

it('keeps inbox storage isolated and waits for the native-close draft save', async () => {
  const { id } = await mount()
  expect(compose$.tabs.peek()).toHaveLength(1)
  let finish!: () => void
  save = () =>
    new Promise<void>((resolve) => {
      finish = resolve
    })
  act(() => handlers.get('composer.closeRequested')!())
  await waitFor(() => expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true))
  expect(compose$.tabs.peek()[0]?.id).toBe(id)
  expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(false)
  expect(localStorage.getItem('meron-compose-tabs')).toBe('[{"id":"inbox-buffer"}]')
  await act(async () => finish())
  await waitFor(() => expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(true))
  expect(calls.find((call) => call.command === 'mail.saveDraft')?.payload.body).toBe('Latest text')
})

it('keeps the editor and content on save failure, and lets closing retry', async () => {
  const { view, id } = await mount()
  save = async () => {
    throw new Error('offline')
  }
  act(() => handlers.get('composer.closeRequested')!())
  await waitFor(() => expect(view.getByText(/Draft autosave failed: offline/).textContent).toContain('offline'))
  expect(compose$.tabs.peek()[0]?.compose?.text).toBe('Latest text')
  expect(getComposeSession(id).savesStopped).toBe(false)
  expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(false)
  save = async () => {}
  act(() => handlers.get('composer.closeRequested')!())
  await waitFor(() => expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(true))
})

it('uses the id allocated by an autosave already running when the window closes', async () => {
  const { id } = await mount()
  let finish!: () => void
  getComposeSession(id).saveChain = new Promise<void>((resolve) => {
    finish = () => {
      updateComposeDraft(id, { draftMessageId: 'in-flight@example.com' })
      resolve()
    }
  })
  act(() => handlers.get('composer.closeRequested')!())
  await act(async () => finish())
  await waitFor(() => expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(true))
  expect(calls.find((call) => call.command === 'mail.saveDraft')?.payload.draft_id).toBe('in-flight@example.com')
  expect(calls.some((call) => call.command === 'mail.allocateIdentity')).toBe(false)
})

it('keeps failed sends open and closes after a successful send', async () => {
  const { view } = await mount()
  send = async () => {
    throw new Error('SMTP unavailable')
  }
  fireEvent.click(view.getByText('Send'))
  await waitFor(() => expect(view.getByText(/SMTP unavailable/)).toBeTruthy())
  expect(compose$.tabs.peek()[0]?.compose?.text).toBe('Latest text')
  expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(false)
  send = async () => {}
  fireEvent.click(view.getByText('Send'))
  await waitFor(() => expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(true))
})

it('hands saved-draft cleanup to the main window before closing after send', async () => {
  const { view, id } = await mount()
  act(() => updateComposeDraft(id, { draftMessageId: 'saved@example.com' }))
  fireEvent.click(view.getByText('Send'))
  await waitFor(() => expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(true))
  const cleanup = calls.findIndex((call) => call.command === 'composer.discardDraft')
  const close = calls.findIndex((call) => call.command === 'composer.closeWindow')
  expect(cleanup).toBeGreaterThan(-1)
  expect(cleanup).toBeLessThan(close)
  expect(calls[cleanup]?.payload.draft.draftMessageId).toBe('saved@example.com')
})

it('closes an untouched message with Ctrl+W without saving an empty draft', async () => {
  const { id } = await mount()
  act(() => updateComposeDraft(id, { to: '', subject: '', text: '' }))
  fireEvent.keyDown(window, { key: 'w', ctrlKey: true })
  await waitFor(() => expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(true))
  expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(false)
})

it('parents attachment file pickers to the standalone composer', async () => {
  const { view } = await mount()
  fireEvent.click(view.getByRole('button', { name: 'Attach files' }))
  await waitFor(() => expect(calls.some((call) => call.command === 'system.pickFiles')).toBe(true))
  expect(calls.find((call) => call.command === 'system.pickFiles')?.payload.compose_window).toBe(true)
})

it('restores a recovery buffer under a separate identity when the inbox owns its draft', async () => {
  const result = await (window as any).go.main.App.Invoke('account.list', {})
  accounts$.set(result.accounts)
  openComposeTab({ draftMessageId: 'already-open@example.com', text: 'Latest text' })
  claim = false
  const recovered = render(
    <StrictMode>
      <ComposeWindow />
    </StrictMode>,
  )
  await waitFor(() => expect(recovered.getByText('Discard')).toBeTruthy())
  expect(
    calls.some(
      (call) => call.command === 'composer.claimDraft' && call.payload.draft_id === 'already-open@example.com',
    ),
  ).toBe(true)
  expect(compose$.tabs.peek()[0]?.compose?.draftMessageId).toMatch(/^local-draft-/)
  expect(compose$.tabs.peek()[0]?.compose?.text).toBe('Latest text')
  recovered.unmount()
})

it('allows a native close when the mounted draft has no sending account', async () => {
  const { id } = await mount()
  act(() => updateComposeDraft(id, { accountId: '' }))
  act(() => handlers.get('composer.closeRequested')!())
  await waitFor(() => expect(calls.some((call) => call.command === 'composer.closeWindow')).toBe(true))
  expect(calls.some((call) => call.command === 'composer.closeResponded')).toBe(true)
})

it('joins a pending startup when focus arrives before accounts finish loading', async () => {
  let finish!: () => void
  readAccounts = () =>
    new Promise<void>((resolve) => {
      finish = resolve
    })
  const view = render(
    <StrictMode>
      <ComposeWindow />
    </StrictMode>,
  )
  fireEvent.focus(window)
  fireEvent.focus(window)
  expect(calls.filter((call) => call.command === 'account.list')).toHaveLength(1)
  await act(async () => finish())
  await waitFor(() => expect(view.getByText('Discard')).toBeTruthy())
  expect(compose$.tabs.peek()).toHaveLength(1)
  expect(calls.some((call) => call.command === 'composer.claimDraft')).toBe(false)
})
