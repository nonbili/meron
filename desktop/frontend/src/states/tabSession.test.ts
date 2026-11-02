import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message, MessageTab } from '../types'
import { moveMessageTab, restoreTabSession } from './compose'
import { COMPOSE_TABS_KEY, compose$ } from './composeState'
import { kanban$ } from './kanban'
import { ui$ } from './ui'

type Page = { messages: Partial<Message>[]; next_cursor?: string }

function mockThreadRead(read: (threadId: string, cursor?: string) => Page | Error) {
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: { thread_id: string; before_cursor?: string }) => {
          if (command !== 'mail.threadRead') return {}
          const result = read(payload.thread_id, payload.before_cursor)
          if (result instanceof Error) throw result
          return result
        },
      },
    },
  }
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 10))
const tab = (id: string, kind: MessageTab['kind'], threadId: string): MessageTab => ({
  id,
  kind,
  messageId: kind === 'reader' ? id : '',
  threadId,
  subject: id,
  from: 'a',
  body: '',
  bodyMissing: kind === 'reader',
  viewMode: 'plain',
})
const message = (id: string, threadId: string): Partial<Message> => ({
  id,
  thread_id: threadId,
  account_id: 'a',
  from_addr: 'a@example.com',
  body: `body of ${id}`,
})

describe('restoreTabSession', () => {
  beforeEach(() => {
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.conversationThread.set('')
    ui$.selectedThread.set('')
    kanban$.activeBoardId.set('')
    kanban$.paneThreadId.set('')
    kanban$.paneColumnKey.set('')
  })
  afterEach(() => {
    delete (window as any).go
    localStorage.removeItem(COMPOSE_TABS_KEY)
  })

  it('reopens an active thread tab, leaving the Current tab behind it', async () => {
    mockThreadRead((id) => ({ messages: [message('m', id)] }))
    compose$.tabs.set([tab('thread-t1', 'thread', 't1')])
    compose$.activeTab.set('thread-t1')
    compose$.conversationThread.set('t2')
    restoreTabSession()
    await settle()
    expect(ui$.selectedThread.peek()).toBe('t1')
    expect(compose$.conversationThread.peek()).toBe('t2')
  })

  it('does not load the conversation behind a reader tab', async () => {
    mockThreadRead((id) => ({ messages: [message('r1', id)] }))
    compose$.tabs.set([tab('r1', 'reader', 't1')])
    compose$.activeTab.set('r1')
    compose$.conversationThread.set('t2')
    restoreTabSession()
    await settle()
    expect(ui$.selectedThread.peek()).toBe('')
    expect(compose$.conversationThread.peek()).toBe('t2')
  })

  it('refetches a reader tab, paging back to find its message', async () => {
    mockThreadRead((id, cursor) =>
      cursor ? { messages: [message('old', id)] } : { messages: [message('new', id)], next_cursor: 'c1' },
    )
    compose$.tabs.set([{ ...tab('old', 'reader', 't1'), viewMode: 'html' }])
    compose$.activeTab.set('old')
    restoreTabSession()
    await settle()
    expect(compose$.tabs.peek()[0]).toMatchObject({ body: 'body of old', viewMode: 'html' })
    expect(compose$.tabs.peek()[0].bodyMissing).toBeFalsy()
    expect(compose$.tabs.peek()[0].message?.id).toBe('old')
  })

  it('closes tabs and the Current conversation whose thread is gone', async () => {
    mockThreadRead(() => ({ messages: [] }))
    const compose = { ...tab('c', 'compose', ''), compose: { to: '' } as MessageTab['compose'] }
    compose$.tabs.set([tab('thread-t1', 'thread', 't1'), tab('r1', 'reader', 't3'), compose])
    compose$.conversationThread.set('t2')
    restoreTabSession()
    await settle()
    expect(compose$.tabs.peek().map((t) => t.id)).toEqual(['c'])
    expect(compose$.conversationThread.peek()).toBe('')
    expect(compose$.activeTab.peek()).toBe('c')
  })

  it('closes a reader tab whose message is gone but keeps its thread tab', async () => {
    mockThreadRead((id) => ({ messages: [message('other', id)] }))
    compose$.tabs.set([tab('thread-t1', 'thread', 't1'), tab('r1', 'reader', 't1')])
    compose$.activeTab.set('thread-t1')
    restoreTabSession()
    await settle()
    expect(compose$.tabs.peek().map((t) => t.id)).toEqual(['thread-t1'])
  })

  it('keeps everything when a read fails', async () => {
    mockThreadRead(() => new Error('offline'))
    compose$.tabs.set([tab('r1', 'reader', 't1')])
    compose$.activeTab.set('r1')
    restoreTabSession()
    await settle()
    expect(compose$.tabs.peek().map((t) => t.id)).toEqual(['r1'])
  })

  it('reopens a board card in the pane', async () => {
    mockThreadRead((id) => ({ messages: [message('m', id)] }))
    kanban$.activeBoardId.set('b')
    kanban$.paneThreadId.set('card')
    restoreTabSession()
    await settle()
    expect(compose$.conversationThread.peek()).toBe('card')
    expect(ui$.selectedThread.peek()).toBe('card')
  })

  it('drops a pane left over from a board that is no longer open', async () => {
    mockThreadRead((id) => ({ messages: [message('m', id)] }))
    kanban$.paneThreadId.set('card')
    restoreTabSession()
    await settle()
    expect(kanban$.paneThreadId.peek()).toBe('')
    expect(ui$.selectedThread.peek()).toBe('')
  })

  it('persists a reordered strip', () => {
    compose$.tabs.set([tab('a', 'reader', 't1'), tab('b', 'thread', 't2'), tab('c', 'reader', 't3')])
    moveMessageTab('c', 'a')
    expect(compose$.tabs.get().map((entry) => entry.id)).toEqual(['c', 'a', 'b'])
    const stored = JSON.parse(localStorage.getItem(COMPOSE_TABS_KEY)!) as { id: string }[]
    expect(stored.map((entry) => entry.id)).toEqual(['c', 'a', 'b'])
  })

  it('persists thread and reader tabs without their bodies', () => {
    compose$.tabs.set([{ ...tab('r1', 'reader', 't1'), body: 'secret', bodyHtml: '<b>secret</b>' }])
    const saved = localStorage.getItem(COMPOSE_TABS_KEY) ?? ''
    expect(JSON.parse(saved)[0]).toMatchObject({ id: 'r1', kind: 'reader', threadId: 't1' })
    expect(saved).not.toContain('secret')
  })
})
