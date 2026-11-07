import { afterEach, beforeEach, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { StrictMode } from 'react'

window.history.replaceState({}, '', '/?threadWindow=session')
localStorage.setItem('meron-compose-tabs', '[{"id":"main-buffer"}]')
const { ThreadWindow } = await import('./ThreadWindow')
const { compose$, COMPOSE_TABS_KEY } = await import('./states/composeState')
const {
  openReplyInFullEditor,
  updateComposeDraft,
  openDraftCompose,
  openComposeTab,
  closeMessageTab,
  closeMessageTabsKeepingDrafts,
} = await import('./states/compose')
const { mail$ } = await import('./states/mail')
const { ui$, dismissToast } = await import('./states/ui')
const { threadView, forgetThreadViews } = await import('./states/threadView')
const { threadData$ } = await import('./states/threadData')
const { settings$, setRemoteImageSender } = await import('./states/settings')
const { archiveThread, deleteMessage } = await import('./states/mailMoves')
const { getComposeSession } = await import('./states/composeSessions')
const { settleConfirm } = await import('./states/ui')
const { openCorrespondentMail } = await import('./states/kanban')

const message = {
  id: 'm1',
  thread_id: 't1',
  account_id: 'a',
  folder_id: 'INBOX',
  subject: 'A conversation',
  from_name: 'Sender',
  from_addr: 'sender@example.com',
  to: 'me@example.com',
  date: 1,
  preview: '',
  body: 'Hello',
  unread: false,
  starred: false,
  has_attachments: false,
  message_id: '<original@example.com>',
}
let calls: { command: string; payload: any }[]
let handlers: Map<string, (detail?: any) => void>
let save: () => Promise<void>
let prefs: Record<string, unknown>
let discard: () => Promise<void>
let reads: number
let scroll: { messageId: string; offset: number; atBottom: boolean } | null
let readThread: (payload: any) => Promise<{ messages: (typeof message)[]; next_cursor?: string }>
beforeEach(() => {
  calls = []
  handlers = new Map()
  save = async () => {}
  reads = 0
  prefs = {}
  discard = async () => {}
  scroll = null
  readThread = async () => ({ messages: [message] })
  compose$.tabs.set([])
  compose$.activeTab.set('')
  ui$.selectedThread.set('')
  mail$.threads.set([])
  mail$.messages.set([])
  threadData$.set({})
  forgetThreadViews()
  dismissToast()
  settings$.conversationLayout.set('traditional')
  ;(window as any).runtime = {
    EventsOn: (name: string, callback: (detail?: any) => void) => {
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
            case 'threadWindow.get':
              return {
                thread_id: 't1',
                thread: message,
                view: {
                  overrides: { m1: true },
                  expandAll: null,
                  unreadOnArrival: [],
                  scroll,
                  revealedRemote: [],
                },
              }
            case 'account.list':
              return { accounts: [{ id: 'a', email: 'me@example.com', provider: 'custom', conversation_html: false }] }
            case 'app.prefsGet':
              return { prefs }
            case 'mail.threadRead':
              reads++
              return readThread(payload)
            case 'mail.allocateIdentity':
              return { message_id: 'draft@example.com' }
            case 'mail.saveDraft':
              await save()
              return {}
            case 'threadWindow.discardDraft':
              await discard()
              return true
            case 'threadWindow.focusDraft':
              return false
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
      <ThreadWindow session="session" />
    </StrictMode>,
  )
  await waitFor(() => expect(view.getByRole('button', { name: 'Copy subject' })).toBeTruthy())
  return view
}
it('boots once in StrictMode, restores view state and isolates main navigation/storage', async () => {
  const view = await mount()
  expect(calls.filter((call) => call.command === 'account.list')).toHaveLength(1)
  expect(threadView('t1').overrides).toEqual({ m1: true })
  expect(COMPOSE_TABS_KEY).toBe('meron-thread-session-tabs')
  expect(localStorage.getItem('meron-compose-tabs')).toBe('[{"id":"main-buffer"}]')
  expect(calls.some((call) => call.command === 'app.prefsSet')).toBe(false)
  expect(view.queryByRole('textbox')).toBeNull()
  const previousReads = reads
  act(() => handlers.get('mail.windowChanged')?.({}))
  await waitFor(() => expect(reads).toBeGreaterThan(previousReads))
})
it('replies locally and waits for the final save before closing its window', async () => {
  await mount()
  act(() => openReplyInFullEditor())
  const tab = compose$.tabs.peek().find((tab) => tab.compose)!
  expect(tab.compose?.to).toContain('sender@example.com')
  act(() => updateComposeDraft(tab.id, { text: 'My reply' }))
  let finish!: () => void
  save = () =>
    new Promise<void>((resolve) => {
      finish = resolve
    })
  act(() => handlers.get('threadWindow.closeRequested')?.({ session: 'other', request: 1 }))
  expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(false)
  act(() => handlers.get('threadWindow.closeRequested')?.({ session: 'session', request: 1 }))
  await waitFor(() => expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true))
  expect(compose$.tabs.peek().some((item) => item.id === tab.id)).toBe(true)
  expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(false)
  expect(calls.findIndex((call) => call.command === 'threadWindow.reserveDraft')).toBeLessThan(
    calls.findIndex((call) => call.command === 'mail.saveDraft'),
  )
  await act(async () => finish())
  await waitFor(() => expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(true))
  expect(calls.find((call) => call.command === 'mail.saveDraft')?.payload.body).toBe('My reply')
})
it('keeps a failed save editable and allows closing to retry', async () => {
  const view = await mount()
  act(() => openReplyInFullEditor())
  const tab = compose$.tabs.peek().find((tab) => tab.compose)!
  act(() => updateComposeDraft(tab.id, { text: 'Keep this reply' }))
  save = async () => {
    throw new Error('offline')
  }
  fireEvent.keyDown(window, { key: 'w', ctrlKey: true, metaKey: true })
  await waitFor(() => expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true))
  await waitFor(() => expect(view.getByText(/offline/)).toBeTruthy())
  expect(compose$.tabs.peek().find((item) => item.id === tab.id)?.compose?.text).toBe('Keep this reply')
  expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(false)
  save = async () => {}
  fireEvent.keyDown(window, { key: 'w', ctrlKey: true, metaKey: true })
  await waitFor(() => expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(true))
})
it('delegates saved drafts to their existing editor instead of hydrating a second reply', async () => {
  await mount()
  await act(async () => {
    await openDraftCompose({ ...message, folder_id: 'Drafts', body: 'Saved reply' })
  })
  expect(calls.some((call) => call.command === 'threadWindow.openDraft')).toBe(true)
  expect(compose$.quickReplyDraftId.peek()).toBe('')
  expect(compose$.tabs.peek().some((tab) => tab.compose)).toBe(false)
})

it('focuses the exact editor tab when another window opens its draft', async () => {
  await mount()
  act(() => openReplyInFullEditor())
  const tab = compose$.tabs.peek().find((tab) => tab.compose)!
  act(() => {
    updateComposeDraft(tab.id, { draftMessageId: 'draft@example.com' })
    compose$.activeTab.set('')
  })
  act(() => handlers.get('threadWindow.focusDraft')?.({ session: 'session', draft_id: '<DRAFT@example.com>' }))
  expect(compose$.activeTab.peek()).toBe(tab.id)
})

it('loads an older saved scroll anchor before mounting the conversation', async () => {
  const older = { ...message, id: 'older', date: 0, body: 'Earlier message' }
  scroll = { messageId: 'older', offset: 0, atBottom: false }
  readThread = async (payload) =>
    payload.before_cursor === 'older-page' ? { messages: [older] } : { messages: [message], next_cursor: 'older-page' }
  const view = await mount()
  expect(calls.some((call) => call.command === 'mail.threadRead' && call.payload.before_cursor === 'older-page')).toBe(
    true,
  )
  expect(threadData$.t1.messages.peek().some((message) => message.id === 'older')).toBe(true)
  expect(view.container.querySelector('[data-message-id="older"]')).toBeTruthy()
})

it('freezes editing during the final save and preserves any newly opened editor', async () => {
  const view = await mount()
  act(() => openReplyInFullEditor())
  let finish!: () => void
  save = () =>
    new Promise<void>((resolve) => {
      finish = resolve
    })
  act(() => handlers.get('threadWindow.closeRequested')?.({ session: 'session', request: 1 }))
  await waitFor(() => expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true))
  expect(view.getByText('Saving draft...')).toBeTruthy()
  expect(view.getByRole('main').getAttribute('aria-busy')).toBe('true')
  let newTab = ''
  act(() => {
    newTab = openComposeTab({ to: 'other@example.com', text: 'A newer reply' })!
  })
  await act(async () => finish())
  expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(false)
  expect(compose$.tabs.peek().find((tab) => tab.id === newTab)?.compose?.text).toBe('A newer reply')
  expect(view.getByRole('main').getAttribute('aria-busy')).toBe('false')
})

it('keeps a partial message deletion on its own thread without loading the mailbox', async () => {
  const older = { ...message, id: 'older', date: 0 }
  readThread = async () => ({ messages: [older, message] })
  await mount()
  readThread = async () => ({ messages: [message] })
  let deletion!: Promise<boolean>
  act(() => {
    deletion = deleteMessage(older)
  })
  await waitFor(() => expect(ui$.confirm.peek()).not.toBeNull())
  act(() => settleConfirm(true))
  await act(async () => {
    await deletion
  })
  expect(ui$.selectedThread.peek()).toBe('t1')
  expect(calls.some((call) => call.command === 'mail.threadList')).toBe(false)
})

it('shows an unavailable conversation after archive instead of choosing another thread', async () => {
  const view = await mount()
  act(() => {
    mail$.threads.set([message, { ...message, id: 'neighbor', thread_id: 'other' }])
  })
  await act(async () => {
    await archiveThread('t1')
  })
  expect(ui$.selectedThread.peek()).toBe('')
  expect(view.getByText('This conversation was moved or deleted.')).toBeTruthy()
  expect(calls.some((call) => call.command === 'mail.threadList')).toBe(false)
  expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(false)
})

it('waits for Discard to reach the main window before allowing the reader to close', async () => {
  await mount()
  act(() => openReplyInFullEditor())
  const tab = compose$.tabs.peek().find((tab) => tab.compose)!
  act(() => updateComposeDraft(tab.id, { draftMessageId: 'draft@example.com', text: 'Discard me' }))
  let finishSave!: () => void
  getComposeSession(tab.id).saveChain = new Promise<void>((resolve) => {
    finishSave = resolve
  })
  let acceptDiscard!: () => void
  discard = () =>
    new Promise<void>((resolve) => {
      acceptDiscard = resolve
    })
  let closingTab!: Promise<void>
  act(() => {
    closingTab = closeMessageTab(tab.id)
  })
  expect(compose$.tabs.peek().some((item) => item.id === tab.id)).toBe(true)
  act(() => handlers.get('threadWindow.closeRequested')?.({ session: 'session', request: 1 }))
  expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(false)
  await act(async () => finishSave())
  await waitFor(() => expect(calls.some((call) => call.command === 'threadWindow.discardDraft')).toBe(true))
  expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(false)
  await act(async () => {
    acceptDiscard()
    await closingTab
  })
  await waitFor(() => expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(true))
})

it('delegates sender allowlist changes and applies the persisted configuration without taking write ownership', async () => {
  await mount()
  act(() => setRemoteImageSender('sender@example.com', true))
  expect(
    calls.some(
      (call) =>
        call.command === 'threadWindow.setRemoteSender' &&
        call.payload.address === 'sender@example.com' &&
        call.payload.allowed,
    ),
  ).toBe(true)
  expect(calls.some((call) => call.command === 'app.prefsSet')).toBe(false)
  prefs = { remote_image_senders: ['sender@example.com', 'other@example.com'] }
  act(() => handlers.get('configuration.changed')?.({ keys: ['remote_image_senders'] }))
  await waitFor(() => expect(settings$.remoteImageSenders.peek()).toEqual(['sender@example.com', 'other@example.com']))
  expect(calls.filter((call) => call.command === 'app.prefsGet').every((call) => !call.payload.session)).toBe(true)
})

it('opens participant mail in the main window without retargeting its conversation', async () => {
  await mount()
  act(() => openCorrespondentMail('a', 'INBOX', 'sender@example.com'))
  expect(ui$.selectedThread.peek()).toBe('t1')
  expect(calls.some((call) => call.command === 'threadWindow.openCorrespondent')).toBe(true)
})

it('retires a removed root conversation while keeping its reply editor alive', async () => {
  await mount()
  act(() => openReplyInFullEditor())
  const tab = compose$.tabs.peek().find((tab) => tab.compose)!
  act(() => updateComposeDraft(tab.id, { text: 'Keep my reply' }))
  readThread = async () => ({ messages: [] })
  act(() => handlers.get('mail.windowChanged')?.({ thread_id: 't1' }))
  await waitFor(() => expect(ui$.selectedThread.peek()).toBe(''))
  expect(compose$.conversationThread.peek()).toBe('')
  expect(compose$.tabs.peek().find((item) => item.id === tab.id)?.compose?.text).toBe('Keep my reply')
  expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(false)
})

it('keeps the root conversation reachable and forwards Open contact to the main window', async () => {
  const { openContact } = await import('./states/contacts')
  const view = await mount()
  act(() => openReplyInFullEditor())
  const current = view.container.querySelector('[data-tab-id=""]')!
  expect(current.querySelector('button[aria-label="Close tab"]')).toBeNull()
  act(() => openContact({ addr: 'sender@example.com', name: 'Sender', count: 1, last_date: 1 } as any))
  expect(
    calls.some(
      (call) => call.command === 'threadWindow.openContact' && call.payload.contact.addr === 'sender@example.com',
    ),
  ).toBe(true)
  expect(ui$.contactsOpen.peek()).toBe(false)
})

it('makes an editor inert while Esc saves its draft, then returns to the conversation', async () => {
  const view = await mount()
  act(() => openReplyInFullEditor())
  const tab = compose$.tabs.peek().find((tab) => tab.compose)!
  act(() => updateComposeDraft(tab.id, { text: 'Keep this text' }))
  let finish!: () => void
  save = () =>
    new Promise<void>((resolve) => {
      finish = resolve
    })
  fireEvent.keyDown(window, { key: 'Escape' })
  await waitFor(() => expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true))
  expect(!!view.getByDisplayValue('Keep this text').closest('[inert]')).toBe(true)
  act(() => compose$.activeTab.set(''))
  act(() => compose$.activeTab.set(tab.id))
  expect(!!view.getByDisplayValue('Keep this text').closest('[inert]')).toBe(true)
  await act(async () => finish())
  await waitFor(() => expect(compose$.tabs.peek().some((item) => item.id === tab.id)).toBe(false))
  expect(ui$.selectedThread.peek()).toBe('t1')
  expect(view.getByText('Saved to Drafts')).toBeTruthy()
  expect(calls.some((call) => call.command === 'threadWindow.close')).toBe(false)
})

it('saves grouped editor tabs without offering an unsafe local draft snapshot reopen', async () => {
  const view = await mount()
  act(() => openReplyInFullEditor())
  const tab = compose$.tabs.peek().find((tab) => tab.compose)!
  act(() => updateComposeDraft(tab.id, { text: 'Keep this grouped draft' }))
  let finish!: () => void
  save = () =>
    new Promise<void>((resolve) => {
      finish = resolve
    })
  act(() => closeMessageTabsKeepingDrafts([tab.id]))
  await waitFor(() => expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true))
  expect(!!view.getByDisplayValue('Keep this grouped draft').closest('[inert]')).toBe(true)
  let closeRequest!: Promise<void>
  act(() => {
    closeRequest = closeMessageTab(tab.id)
  })
  await act(async () => {
    await Promise.resolve()
  })
  expect(calls.some((call) => call.command === 'threadWindow.discardDraft')).toBe(false)
  expect(compose$.tabs.peek().some((item) => item.id === tab.id)).toBe(true)
  await act(async () => {
    finish()
    await closeRequest
  })
  await waitFor(() => expect(compose$.tabs.peek().some((item) => item.id === tab.id)).toBe(false))
  expect(view.getByText('Saved to Drafts')).toBeTruthy()
  expect(view.queryByRole('button', { name: 'Open draft' })).toBeNull()
})
