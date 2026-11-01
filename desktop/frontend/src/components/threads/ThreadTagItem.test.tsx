import { afterEach, expect, it } from 'bun:test'
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import type { Message } from '../../types'
import { mail$ } from '../../states/mail'
import { ui$ } from '../../states/ui'
import { ThreadTagItem } from './ThreadTagItem'

type SetTags = { thread_id: string; add: string[]; remove: string[] }

const THREAD = 'acc#INBOX#thread'

// A core holding `tags`, whose mail.setTags answers wait for `gate` when given.
function stubCore(tags: string[], counts: { tag: string; count: number }[], gate?: Promise<void>) {
  const calls: SetTags[] = []
  let held = tags
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: SetTags) => {
          if (command === 'mail.tagList') return { tags: counts }
          if (command !== 'mail.setTags') return {}
          calls.push(payload)
          await gate
          held = [...held.filter((tag) => !payload.remove.includes(tag)), ...payload.add].sort()
          return { tags: held }
        },
      },
    },
  }
  mail$.threads.set([{ thread_id: THREAD, tags } as Message])
  return calls
}

const flyoutRows = () =>
  Array.from(document.querySelectorAll('[data-thread-context-menu] button')).map((button) => [
    button.textContent,
    button.getAttribute('aria-pressed'),
  ])

afterEach(() => {
  cleanup()
  delete (window as any).go
  mail$.threads.set([])
  ui$.tagThreadId.set('')
})

it('lists the thread tags, then the most used others, and keeps rows in place across toggles', async () => {
  const calls = stubCore(
    ['work'],
    [
      { tag: 'rare', count: 1 },
      { tag: 'work', count: 5 },
      { tag: 'common', count: 9 },
    ],
  )
  const view = render(<ThreadTagItem key={THREAD} threadId={THREAD} close={() => {}} />)
  await waitFor(() => expect(view.getByRole('button').getAttribute('aria-haspopup')).toBe('menu'))
  fireEvent.click(view.getByText('Tags'))
  await waitFor(() =>
    expect(flyoutRows()).toEqual([
      ['work', 'true'],
      ['common', 'false'],
      ['rare', 'false'],
      ['Add a tag', null],
    ]),
  )

  fireEvent.click(view.getByText('common'))
  await waitFor(() => expect(flyoutRows()[1]).toEqual(['common', 'true']))
  fireEvent.click(view.getByText('work'))
  await waitFor(() => expect(flyoutRows()[0]).toEqual(['work', 'false']))

  expect(calls.map(({ add, remove }) => ({ add, remove }))).toEqual([
    { add: ['common'], remove: [] },
    { add: [], remove: ['work'] },
  ])
  expect(flyoutRows().map(([label]) => label)).toEqual(['work', 'common', 'rare', 'Add a tag'])
})

it('lets two quick clicks on one row cancel out', async () => {
  let release = () => {}
  const gate = new Promise<void>((resolve) => {
    release = resolve
  })
  const calls = stubCore([], [{ tag: 'common', count: 9 }], gate)
  const view = render(<ThreadTagItem key={THREAD} threadId={THREAD} close={() => {}} />)
  await waitFor(() => expect(view.getByRole('button').getAttribute('aria-haspopup')).toBe('menu'))
  fireEvent.click(view.getByText('Tags'))
  await waitFor(() => expect(flyoutRows().length).toBe(2))

  fireEvent.click(view.getByText('common'))
  fireEvent.click(view.getByText('common'))
  release()

  await waitFor(() => expect(calls.length).toBe(2))
  expect(calls.map(({ add, remove }) => ({ add, remove }))).toEqual([
    { add: ['common'], remove: [] },
    { add: [], remove: ['common'] },
  ])
  await waitFor(() => expect(flyoutRows()[0]).toEqual(['common', 'false']))
})

it('stays open while the pointer crosses to the flyout, and closes once it has left', async () => {
  stubCore(['work'], [])
  const view = render(<ThreadTagItem key={THREAD} threadId={THREAD} close={() => {}} />)
  const row = view.getByText('Tags').closest('div') as HTMLElement
  fireEvent.mouseEnter(row)
  await waitFor(() => expect(flyoutRows().length).toBe(2))

  fireEvent.mouseLeave(row)
  fireEvent.mouseEnter(row)
  await new Promise((resolve) => setTimeout(resolve, 250))
  expect(flyoutRows().length).toBe(2)

  fireEvent.mouseLeave(row)
  await waitFor(() => expect(flyoutRows().length).toBe(0))
})

it('lets a click from a reopened menu cancel out one still in flight', async () => {
  let release = () => {}
  const gate = new Promise<void>((resolve) => {
    release = resolve
  })
  const calls = stubCore([], [{ tag: 'common', count: 9 }], gate)
  const open = async () => {
    const view = render(<ThreadTagItem key={THREAD} threadId={THREAD} close={() => {}} />)
    await waitFor(() => expect(view.getByRole('button').getAttribute('aria-haspopup')).toBe('menu'))
    fireEvent.click(view.getByText('Tags'))
    await waitFor(() => expect(flyoutRows().length).toBe(2))
    fireEvent.click(view.getByText('common'))
    return view
  }
  ;(await open()).unmount()
  await open()
  release()

  await waitFor(() => expect(calls.length).toBe(2))
  expect(calls.map(({ add, remove }) => ({ add, remove }))).toEqual([
    { add: ['common'], remove: [] },
    { add: [], remove: ['common'] },
  ])
})

it('keeps toggling from its own answers once the thread has left every view', async () => {
  const calls = stubCore(['work'], [])
  const view = render(<ThreadTagItem key={THREAD} threadId={THREAD} close={() => {}} />)
  fireEvent.click(view.getByText('Tags'))
  await waitFor(() => expect(flyoutRows().length).toBe(2))

  fireEvent.click(view.getByText('work'))
  await waitFor(() => expect(flyoutRows()[0]).toEqual(['work', 'false']))
  mail$.threads.set([])
  fireEvent.click(view.getByText('work'))
  await waitFor(() => expect(flyoutRows()[0]).toEqual(['work', 'true']))
  fireEvent.click(view.getByText('work'))
  await waitFor(() => expect(flyoutRows()[0]).toEqual(['work', 'false']))

  expect(calls.map(({ add, remove }) => ({ add, remove }))).toEqual([
    { add: [], remove: ['work'] },
    { add: ['work'], remove: [] },
    { add: [], remove: ['work'] },
  ])
})

it('opens the tag dialog from the row when there is nothing to list', async () => {
  stubCore([], [])
  let closed = 0
  const view = render(<ThreadTagItem key={THREAD} threadId={THREAD} close={() => closed++} />)
  fireEvent.click(view.getByText('Tags'))
  expect(closed).toBe(1)
  expect(ui$.tagThreadId.peek()).toBe(THREAD)
})

it('hands adding a tag to the dialog', async () => {
  stubCore(['work'], [])
  let closed = 0
  const view = render(<ThreadTagItem key={THREAD} threadId={THREAD} close={() => closed++} />)
  fireEvent.click(view.getByText('Tags'))
  await waitFor(() => expect(flyoutRows().length).toBe(2))
  fireEvent.click(view.getByText('Add a tag'))
  expect(closed).toBe(1)
  expect(ui$.tagThreadId.peek()).toBe(THREAD)
})
