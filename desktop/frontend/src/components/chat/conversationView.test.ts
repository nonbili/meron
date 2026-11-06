import { afterEach, expect, it } from 'bun:test'
import { bridgeConversationState, createConversationState } from './conversationView'
import {
  forgetRemoteReveals,
  openThreadSearch,
  revealRemote,
  setAllMessagesExpanded,
  thread$,
} from '../../states/thread'

afterEach(() => {
  forgetRemoteReveals()
  thread$.search.set('')
  thread$.searchOpen.set(false)
  thread$.expandAll.set(null)
})

it('routes commands to the active view and preserves hidden search and expansion state', () => {
  const a = createConversationState()
  const b = createConversationState()
  let disconnect = bridgeConversationState(a.local)
  openThreadSearch()
  thread$.search.set('first query')
  setAllMessagesExpanded(true)
  expect(a.state.search.peek()).toBe('first query')
  expect(a.state.expandAll.peek()?.expanded).toBe(true)
  expect(b.state.search.peek()).toBe('')
  expect(b.state.expandAll.peek()).toBeNull()
  disconnect()
  disconnect = bridgeConversationState(b.local)
  thread$.search.set('second query')
  setAllMessagesExpanded(false)
  expect(a.state.search.peek()).toBe('first query')
  expect(a.state.expandAll.peek()?.expanded).toBe(true)
  disconnect()
  disconnect = bridgeConversationState(a.local)
  expect(thread$.search.peek()).toBe('first query')
  expect(thread$.searchOpen.peek()).toBe(true)
  expect(thread$.expandAll.peek()?.expanded).toBe(true)
  // Only the active view publishes search counts to external commands.
  b.state.searchMatchCount.set(30)
  expect(thread$.searchMatchCount.peek()).toBe(a.state.searchMatchCount.peek())
  disconnect()
})

it('reveals remote content only in the active mounted view, while preserving reveals for navigation', () => {
  const a = createConversationState()
  const b = createConversationState()
  let disconnect = bridgeConversationState(a.local)
  let notifications = 0
  const off = a.local.revealedRemote.onChange(() => {
    notifications++
  })
  revealRemote('message')
  expect(notifications).toBe(1)
  off()
  expect(a.state.revealedRemote.message.peek()).toBe(true)
  expect(b.state.revealedRemote.message.peek()).toBeUndefined()
  disconnect()
  disconnect = bridgeConversationState(b.local)
  expect(b.state.revealedRemote.message.peek()).toBeUndefined()
  revealRemote('message')
  expect(b.state.revealedRemote.message.peek()).toBe(true)
  disconnect()
  const navigated = createConversationState()
  expect(navigated.state.revealedRemote.message.peek()).toBe(true)
})

it('does not repeat expand-all when unrelated interaction state changes', () => {
  const view = createConversationState()
  const disconnect = bridgeConversationState(view.local)
  let commands = 0
  const off = view.local.expandAll.onChange(() => {
    commands++
  })
  setAllMessagesExpanded(true)
  thread$.hoveredLink.set('https://example.com')
  revealRemote('image')
  expect(commands).toBe(1)
  off()
  disconnect()
  thread$.hoveredLink.set(null)
})
