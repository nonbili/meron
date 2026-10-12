import { afterEach, beforeEach, expect, it } from 'bun:test'
import { accounts$ } from './accounts'
import { compose$ } from './composeState'
import { thread$ } from './thread'
import type { Account, MessageTab } from '../types'
import { toggleConversationMode } from './compose'

const account = (id: string, extra: Partial<Account> = {}): Account => ({
  id,
  email: `${id}@example.com`,
  display_name: id,
  provider: 'custom',
  auth_type: 'password',
  imap_host: 'imap.example.com',
  imap_port: 993,
  smtp_host: 'smtp.example.com',
  smtp_port: 465,
  tls: true,
  ...extra,
})

const tab = (kind: MessageTab['kind'], extra: Partial<MessageTab> = {}): MessageTab => ({
  id: `${kind}-tab`,
  kind,
  messageId: 'm1',
  threadId: 't1',
  subject: 'Subject',
  from: 'a@example.com',
  body: 'Body',
  viewMode: 'html',
  ...extra,
})

// Other test files leave tabs and overrides behind, so start from a clean
// slate as well as leaving one.
function resetState() {
  accounts$.set([])
  thread$.conversationModeOverrides.set({})
  compose$.tabs.set([])
  compose$.activeTab.set('')
}

beforeEach(() => {
  resetState()
  accounts$.set([account('html-acc'), account('plain-acc', { conversation_html: false })])
})

afterEach(resetState)

it('flips an HTML account to plain text and back', () => {
  expect(toggleConversationMode('html-acc')).toBe(true)
  expect(thread$.conversationModeOverrides.peek()['html-acc']).toBe('plain')
  expect(toggleConversationMode('html-acc')).toBe(true)
  expect(thread$.conversationModeOverrides.peek()['html-acc']).toBe('html')
})

it('flips an account set to plain text to HTML', () => {
  expect(toggleConversationMode('plain-acc')).toBe(true)
  expect(thread$.conversationModeOverrides.peek()['plain-acc']).toBe('html')
})

it('does nothing without a known account', () => {
  expect(toggleConversationMode(undefined)).toBe(false)
  expect(toggleConversationMode('missing')).toBe(false)
  expect(thread$.conversationModeOverrides.peek()).toEqual({})
})

it('flips the reader tab in front instead of the conversation', () => {
  compose$.tabs.set([tab('reader', { bodyHtml: '<p>Body</p>' })])
  compose$.activeTab.set('reader-tab')
  expect(toggleConversationMode('html-acc')).toBe(true)
  expect(compose$.tabs.peek()[0].viewMode).toBe('plain')
  expect(thread$.conversationModeOverrides.peek()).toEqual({})
  toggleConversationMode('html-acc')
  expect(compose$.tabs.peek()[0].viewMode).toBe('html')
})

it('leaves a reader tab with no HTML body alone', () => {
  compose$.tabs.set([tab('reader', { viewMode: 'plain' })])
  compose$.activeTab.set('reader-tab')
  expect(toggleConversationMode('html-acc')).toBe(false)
  expect(compose$.tabs.peek()[0].viewMode).toBe('plain')
  expect(thread$.conversationModeOverrides.peek()).toEqual({})
})

it("uses a thread tab's own account", () => {
  compose$.tabs.set([tab('thread', { accountId: 'plain-acc' })])
  compose$.activeTab.set('thread-tab')
  expect(toggleConversationMode('html-acc')).toBe(true)
  expect(thread$.conversationModeOverrides.peek()).toEqual({ 'plain-acc': 'html' })
})

it('leaves a compose tab alone', () => {
  compose$.tabs.set([tab('compose')])
  compose$.activeTab.set('compose-tab')
  expect(toggleConversationMode('html-acc')).toBe(false)
  expect(thread$.conversationModeOverrides.peek()).toEqual({})
})
