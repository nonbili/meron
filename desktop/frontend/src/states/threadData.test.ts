import { afterEach, expect, it } from 'bun:test'
import { getActiveThread, loadThread, loadMoreMessages, mail$ } from './mail'
import { ensureThreadData, threadData$, threadRequests } from './threadData'
import { compose$ } from './composeState'
import { ui$ } from './ui'
import type { Message } from '../types'

const message = (thread_id: string, id = thread_id, date = 1): Message => ({
  thread_id,
  id,
  date,
  account_id: 'acc',
  folder_id: 'inbox',
  from_name: 'A',
  from_addr: 'a@example.com',
  to: '',
  subject: 'Subject',
  preview: '',
  body: 'Body',
  unread: false,
  starred: false,
  has_attachments: false,
})
afterEach(() => {
  mail$.messages.set([])
  threadData$.set({})
  threadRequests.clear()
  ui$.selectedThread.set('')
  compose$.tabs.set([])
  compose$.activeTab.set('')
  delete (window as any).go
})
function backend(read: (payload: any) => any) {
  ;(window as any).go = { main: { App: { Invoke: (_command: string, payload: any) => read(payload) } } }
}

it('accepts hidden-thread results without replacing the foreground conversation', async () => {
  let finish!: (result: unknown) => void
  backend(({ thread_id }) =>
    thread_id === 'a'
      ? new Promise((resolve) => {
          finish = resolve
        })
      : { messages: [message('b')] },
  )
  ui$.selectedThread.set('a')
  const pending = loadThread('a')
  ui$.selectedThread.set('b')
  await loadThread('b')
  finish({ messages: [message('a')], next_cursor: 'older-a' })
  await pending
  expect(mail$.messages.peek().map((item) => item.thread_id)).toEqual(['b'])
  expect(threadData$.a.messages.peek().map((item) => item.thread_id)).toEqual(['a'])
  expect(threadData$.a.cursor.peek()).toBe('older-a')
  expect(threadData$.a.loading.peek()).toBe(false)
  // Foreground edits don't erase hidden caches.
  mail$.messages[0].starred.set(true)
  expect(threadData$.b.messages[0].starred.peek()).toBe(true)
  expect(threadData$.a.messages.peek()).toHaveLength(1)
})

it('preserves paginated history and its cursor on activation refresh', async () => {
  const data = ensureThreadData('a')
  data.assign({ messages: [message('a', 'old', 1), message('a', 'new', 3)], cursor: 'oldest' })
  backend(() => ({ messages: [message('a', 'new', 3)], next_cursor: 'newest-page' }))
  ui$.selectedThread.set('a')
  await loadThread('a')
  expect(data.messages.peek().map((item) => item.id)).toEqual(['old', 'new'])
  expect(data.cursor.peek()).toBe('oldest')
})

it('finishes pagination in the owning cache when the user switches tabs', async () => {
  const data = ensureThreadData('a')
  data.assign({ messages: [message('a', 'new', 3)], cursor: 'earlier' })
  let finish!: (result: unknown) => void
  backend(
    () =>
      new Promise((resolve) => {
        finish = resolve
      }),
  )
  ui$.selectedThread.set('a')
  const pending = loadMoreMessages('a')
  ui$.selectedThread.set('b')
  mail$.messages.set([message('b')])
  finish({ messages: [message('a', 'old', 1)] })
  await pending
  expect(data.messages.peek().map((item) => item.id)).toEqual(['old', 'new'])
  expect(data.loadingMore.peek()).toBe(false)
  expect(mail$.messages.peek()[0].thread_id).toBe('b')
})

it('ignores an older refresh response for the same thread', async () => {
  const finish: ((result: unknown) => void)[] = []
  backend(() => new Promise((resolve) => finish.push(resolve)))
  ui$.selectedThread.set('a')
  const first = loadThread('a')
  const second = loadThread('a')
  finish[1]({ messages: [message('a', 'newer')] })
  await second
  finish[0]({ messages: [message('a', 'stale')] })
  await first
  expect(threadData$.a.messages[0].id.peek()).toBe('newer')
})

it('resolves a pinned conversation for actions before its list row or body has loaded', () => {
  compose$.tabs.set([
    {
      id: 'pin',
      kind: 'thread',
      threadId: 'outside-list',
      messageId: '',
      accountId: 'other-account',
      folderId: 'sent',
      subject: 'Pinned subject',
      from: 'Sender',
      body: '',
      viewMode: 'plain',
    },
  ])
  compose$.activeTab.set('pin')
  ui$.selectedThread.set('outside-list')
  expect(getActiveThread()?.thread_id).toBe('outside-list')
  expect(getActiveThread()?.account_id).toBe('other-account')
})
