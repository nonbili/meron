import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { accounts$ } from './accounts'
import { removeFeed } from './feeds'
import { mail$ } from './mail'
import { ui$ } from './ui'

const feed = (overrides: Partial<Message> = {}): Message => ({
  id: 'rss-account:rss:feed-1#item-1',
  account_id: 'rss-account',
  folder_id: 'rss',
  thread_id: 'rss-account#rss#feed-1',
  from_name: 'Example Feed',
  from_addr: 'https://example.com/feed.xml',
  to: '',
  subject: 'Example Feed',
  preview: 'Latest item',
  body: '',
  date: 1,
  unread: true,
  starred: false,
  has_attachments: false,
  ...overrides,
})

describe('removeFeed', () => {
  const calls: { command: string; payload: unknown }[] = []

  beforeEach(() => {
    calls.length = 0
    accounts$.set([
      {
        id: 'rss-account',
        email: 'rss-account.local',
        display_name: 'Feeds',
        provider: 'rss',
        auth_type: 'rss',
        imap_host: '',
        imap_port: 0,
        smtp_host: '',
        smtp_port: 0,
        tls: false,
        signature: '',
      },
    ])
    mail$.threads.set([feed()])
    mail$.threadsCursor.set('older-page')
    mail$.threadAccountCursors.set({})
    mail$.folders.set([{ id: 'rss', account_id: 'rss-account', name: 'Feeds', role: 'inbox', unread: 1 }])
    mail$.foldersByAccount.set({})
    ui$.selectedAccount.set('rss-account')
    ui$.selectedFolder.set('rss')
    ui$.selectedThread.set('rss-account#rss#feed-1')
    ui$.selectedStarredItem.set('')
    ui$.query.set('')
    ui$.filterMode.set('all')
    ui$.editFeed.set({ threadId: 'rss-account#rss#feed-1', name: 'Example Feed' })
    ui$.toast.set('')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.threadList') return { threads: [] }
            if (command === 'mail.folderList') {
              return {
                folders: [{ id: 'rss', account_id: 'rss-account', name: 'Feeds', role: 'inbox', unread: 0 }],
              }
            }
            return { ok: true }
          },
        },
      },
    }
  })

  it('removes the deleted feed from the visible thread list', async () => {
    await removeFeed('rss-account#rss#feed-1')

    expect(calls.find((call) => call.command === 'mail.threadList')?.payload).toMatchObject({ refresh: true })
    expect(mail$.threads.get()).toEqual([])
    expect(ui$.selectedThread.get()).toBe('')
    expect(ui$.editFeed.get()).toBeNull()
  })
})
