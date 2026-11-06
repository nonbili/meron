import { afterEach, beforeEach, expect, it } from 'bun:test'
import { act, cleanup, render, waitFor } from '@testing-library/react'
import { MessagePane } from './MessagePane'
import {
  activateConversationTab,
  openThreadTab,
  openMessageTab,
  openComposeTab,
  finishClosingMessageTab,
} from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { accounts$ } from '../../states/accounts'
import { pageOpenMessage } from './messagePaging'
import { mail$ } from '../../states/mail'
import { threadData$, threadRequests } from '../../states/threadData'
import { thread$, openThreadSearch, revealRemote, setAllMessagesExpanded } from '../../states/thread'
import { forgetThreadViews, threadView } from '../../states/threadView'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import type { Account, Message } from '../../types'

const message = (thread_id: string): Message => ({
  thread_id,
  id: thread_id,
  date: 1,
  account_id: 'local',
  folder_id: 'inbox',
  from_name: 'A',
  from_addr: 'a@example.com',
  to: '',
  subject: thread_id,
  preview: '',
  body: 'Body',
  body_html: '<p>Body</p>',
  unread: false,
  starred: false,
  has_attachments: false,
})
const originalObserver = globalThis.IntersectionObserver
const a = message('a')
const b = message('b')

beforeEach(() => {
  globalThis.IntersectionObserver = undefined as unknown as typeof IntersectionObserver
  accounts$.set([
    {
      id: 'local',
      email: 'me@example.com',
      display_name: 'Me',
      provider: 'custom',
      auth_type: 'password',
      imap_host: '',
      imap_port: 993,
      smtp_host: '',
      smtp_port: 465,
      tls: true,
      conversation_html: true,
    } as Account,
  ])
  compose$.tabs.set([])
  compose$.activeTab.set('')
  mail$.threads.set([a, b])
  ui$.selectedThread.set('a')
  settings$.conversationLayout.set('traditional')
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) =>
          command === 'mail.threadRead' ? { messages: [message(payload.thread_id)] } : { ok: true },
      },
    },
  }
})
afterEach(async () => {
  cleanup()
  globalThis.IntersectionObserver = originalObserver
  accounts$.set([])
  await Promise.resolve()
  compose$.tabs.set([])
  compose$.activeTab.set('')
  ui$.selectedThread.set('')
  mail$.messages.set([])
  mail$.threads.set([])
  threadData$.set({})
  threadRequests.clear()
  forgetThreadViews()
  settings$.conversationLayout.set('chat')
  delete (window as any).go
})

it('keeps Current and pinned scrollers mounted with independent search state', async () => {
  const view = render(<MessagePane />)
  await waitFor(() => expect(view.container.querySelectorAll('[data-message-id]').length).toBe(1))
  const current = view.container.querySelector<HTMLElement>('[data-message-scroller]')!
  const frame = current.querySelector('iframe')!
  const source = frame.getAttribute('srcdoc')
  current.scrollTop = 123
  act(() => {
    openThreadSearch()
    thread$.search.set('Body')
  })
  await act(async () => openThreadTab(b))
  await waitFor(() => expect(view.container.querySelectorAll('[data-message-scroller]').length).toBe(2))
  const pinned = view.container.querySelectorAll<HTMLElement>('[data-message-scroller]')[1]
  expect(current.closest('[aria-hidden="true"]') !== null).toBe(true)
  expect(current.isConnected).toBe(true)
  expect(thread$.search.peek()).toBe('')
  act(() => {
    thread$.search.set('different')
    setAllMessagesExpanded(false)
  })
  pinned.scrollTop = 234
  await act(async () => activateConversationTab())
  expect(view.container.querySelector('[data-message-scroller]') === current).toBe(true)
  expect(current.scrollTop).toBe(123)
  expect(current.querySelector('iframe') === frame).toBe(true)
  expect(frame.getAttribute('srcdoc')).toBe(source)
  expect(pinned.scrollTop).toBe(234)
  expect(pinned.closest('[aria-hidden="true"]') !== null).toBe(true)
  expect(thread$.search.peek()).toBe('Body')
  await act(async () => openThreadTab(b))
  expect(thread$.search.peek()).toBe('different')
  expect(view.container.querySelectorAll('[data-message-scroller]')[1] === pinned).toBe(true)
})

it('unmounts the old Current conversation on list navigation while a pinned view stays alive', async () => {
  const view = render(<MessagePane />)
  await waitFor(() => expect(view.container.querySelectorAll('[data-message-id]').length).toBe(1))
  const current = view.container.querySelector('[data-message-scroller]')!
  await act(async () => openThreadTab(b))
  const pinned = view.container.querySelectorAll('[data-message-scroller]')[1]
  await act(async () => activateConversationTab())
  act(() => ui$.selectedThread.set('b'))
  expect(current.isConnected).toBe(false)
  expect(pinned.isConnected).toBe(true)
})

it('hides every conversation under reader and compose tabs without replacing their DOM', async () => {
  const view = render(<MessagePane />)
  await waitFor(() => expect(view.container.querySelectorAll('[data-message-id]').length).toBe(1))
  await act(async () => openThreadTab(b))
  const scrollers = [...view.container.querySelectorAll<HTMLElement>('[data-message-scroller]')]
  const frames = scrollers.map((scroller) => scroller.querySelector('iframe'))
  await act(async () => openMessageTab(b))
  for (const scroller of scrollers) {
    expect(scroller.isConnected).toBe(true)
    expect(scroller.closest('[aria-hidden="true"]') !== null).toBe(true)
  }
  // Commands wait for a foreground conversation; hidden views cannot consume them.
  act(() => thread$.pendingScrollMessageId.set('a'))
  expect(thread$.pendingScrollMessageId.peek()).toBe('a')
  await act(async () => {
    openComposeTab({ accountId: 'local' })
  })
  scrollers.forEach((scroller, index) => expect(scroller.querySelector('iframe') === frames[index]).toBe(true))
  await act(async () => {
    activateConversationTab()
  })
  expect(thread$.pendingScrollMessageId.peek()).toBe('')
  expect(scrollers[0].closest('[aria-hidden="true"]') === null).toBe(true)
  // A hidden, laid-out scroller must never be the keyboard paging target.
  let currentPages = 0
  let hiddenPages = 0
  for (const scroller of scrollers) Object.defineProperty(scroller, 'getClientRects', { value: () => [{}] })
  scrollers[0].scrollBy = () => {
    currentPages++
  }
  scrollers[1].scrollBy = () => {
    hiddenPages++
  }
  pageOpenMessage(1)
  expect(currentPages).toBe(1)
  expect(hiddenPages).toBe(0)
})

it('keeps a single quick-reply editor across conversation and reader switches', async () => {
  accounts$[0].id.set('acc')
  const first = { ...a, account_id: 'acc' }
  const second = { ...b, account_id: 'acc' }
  mail$.threads.set([first, second])
  ;(window as any).go.main.App.Invoke = async (command: string, payload: any) =>
    command === 'mail.threadRead' ? { messages: [{ ...message(payload.thread_id), account_id: 'acc' }] } : { ok: true }
  const view = render(<MessagePane />)
  await waitFor(() => expect(view.container.querySelectorAll('[data-message-id]').length).toBe(1))
  const editor = view.container.querySelector('textarea')!
  expect(editor !== null).toBe(true)
  await act(async () => openThreadTab(second))
  expect(view.container.querySelectorAll('textarea').length).toBe(1)
  expect(view.container.querySelector('textarea') === editor).toBe(true)
  await act(async () => openMessageTab(second))
  expect(view.container.querySelector('textarea') === editor).toBe(true)
  expect(editor.closest('[aria-hidden="true"]') !== null).toBe(true)
  await act(async () => activateConversationTab())
  expect(view.container.querySelector('textarea') === editor).toBe(true)
})

it('keeps two views of the same thread independent when revealing content and closing a tab', async () => {
  const view = render(<MessagePane />)
  await waitFor(() => expect(view.container.querySelectorAll('[data-message-id]').length).toBe(1))
  const current = view.container.querySelector<HTMLElement>('[data-message-scroller]')!
  const frame = current.querySelector('iframe')!
  const source = frame.getAttribute('srcdoc')
  current.scrollTop = 123
  await act(async () => openThreadTab(a))
  const pinId = compose$.activeTab.peek()
  const scope = `${pinId}:a`
  threadView(scope).overrides = { marker: true }
  const pinned = view.container.querySelectorAll<HTMLElement>('[data-message-scroller]')[1]
  pinned.scrollTop = 234
  act(() => revealRemote('a'))
  expect(current.querySelector('iframe') === frame).toBe(true)
  expect(frame.getAttribute('srcdoc')).toBe(source)
  await act(async () => finishClosingMessageTab(pinId))
  expect(current.isConnected).toBe(true)
  expect(pinned.isConnected).toBe(false)
  expect(threadView(scope).overrides).toEqual({})
  expect(current.scrollTop).toBe(123)
  expect(frame.getAttribute('srcdoc')).toBe(source)
})

it('refreshes a cached conversation on activation without rereading hidden tabs', async () => {
  const reads: string[] = []
  let body = 'Before activation'
  ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
    if (command === 'mail.threadRead') {
      reads.push(payload.thread_id)
      return { messages: [{ ...message(payload.thread_id), body }] }
    }
    return { ok: true }
  }
  render(<MessagePane />)
  await waitFor(() => expect(threadData$.a.messages.peek()?.length).toBe(1))
  await act(async () => openThreadTab(b))
  const aReads = reads.filter((id) => id === 'a').length
  const bReads = reads.filter((id) => id === 'b').length
  body = 'After activation'
  await act(async () => activateConversationTab())
  expect(reads.filter((id) => id === 'a').length).toBe(aReads + 1)
  expect(reads.filter((id) => id === 'b').length).toBe(bReads)
  expect(threadData$.a.messages[0].body.peek()).toBe('After activation')
})
