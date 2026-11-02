import { afterEach, describe, expect, it } from 'bun:test'
import { COMPOSE_TABS_KEY, TAB_VIEW_KEY } from './composeState'

// compose$ reads the persisted tabs once at import, so each case boots a fresh copy.
async function boot(name: string) {
  const { compose$ } = await import(`./composeState?${name}`)
  return compose$
}

describe('compose$ startup', () => {
  afterEach(() => {
    localStorage.removeItem(COMPOSE_TABS_KEY)
    localStorage.removeItem(TAB_VIEW_KEY)
  })

  it('activates the last restored compose tab', async () => {
    const tab = (id: string) => ({ id, subject: id, compose: { to: '' } })
    localStorage.setItem(COMPOSE_TABS_KEY, JSON.stringify([tab('compose-1'), tab('compose-2')]))
    const compose$ = await boot('restored')
    expect(compose$.tabs.peek().map((t: { id: string }) => t.id)).toEqual(['compose-1', 'compose-2'])
    expect(compose$.activeTab.peek()).toBe('compose-2')
  })

  it('restores thread and reader tabs in order, with the saved view', async () => {
    const mail = (id: string, kind: string) => ({
      id,
      kind,
      messageId: kind === 'reader' ? id : '',
      threadId: 't1',
      subject: id,
      from: 'a',
      viewMode: 'html',
    })
    localStorage.setItem(
      COMPOSE_TABS_KEY,
      JSON.stringify([
        mail('thread-t1', 'thread'),
        { id: 'c', subject: '', compose: { to: '' } },
        mail('m1', 'reader'),
      ]),
    )
    localStorage.setItem(TAB_VIEW_KEY, JSON.stringify({ activeTab: 'm1', conversationThread: 't2' }))
    const compose$ = await boot('view')
    expect(compose$.tabs.peek().map((t: { kind: string }) => t.kind)).toEqual(['thread', 'compose', 'reader'])
    expect(compose$.tabs.peek()[2]).toMatchObject({ body: '', bodyMissing: true, viewMode: 'html' })
    expect(compose$.activeTab.peek()).toBe('m1')
    expect(compose$.conversationThread.peek()).toBe('t2')
  })

  it('returns to the Current tab when it was active', async () => {
    localStorage.setItem(COMPOSE_TABS_KEY, JSON.stringify([{ id: 'c', subject: '', compose: { to: '' } }]))
    localStorage.setItem(TAB_VIEW_KEY, JSON.stringify({ activeTab: '', conversationThread: 't2' }))
    const compose$ = await boot('current')
    expect(compose$.activeTab.peek()).toBe('')
  })

  it('stays on the conversation view with nothing to restore', async () => {
    const compose$ = await boot('empty')
    expect(compose$.activeTab.peek()).toBe('')
  })
})
