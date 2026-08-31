import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, renderHook } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { cancelQuickReplyDraftSave, compose$ } from '../../states/compose'
import { mail$ } from '../../states/mail'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import type { Message } from '../../types'
import { useQuickReply } from './useQuickReply'

const thread: Message = {
  id: 'root',
  account_id: 'acc-1',
  folder_id: 'INBOX',
  thread_id: 't-1',
  from_name: 'Them',
  from_addr: 'them@example.com',
  to: 'me@example.com',
  message_id: 'root@example.com',
  subject: 'Subject',
  preview: 'Original',
  body: 'Original',
  date: 1,
  unread: false,
  starred: false,
  has_attachments: false,
}

describe('useQuickReply draft ownership', () => {
  const calls: Array<{ command: string; payload: unknown }> = []
  let previousGo: unknown

  beforeEach(() => {
    previousGo = (window as any).go
    cancelQuickReplyDraftSave()
    calls.length = 0
    accounts$.set([
      {
        id: 'acc-1',
        email: 'me@example.com',
        display_name: 'Me',
        provider: 'custom',
        auth_type: 'password',
        imap_host: 'imap.example.com',
        imap_port: 993,
        smtp_host: 'smtp.example.com',
        smtp_port: 465,
        tls: true,
      },
    ])
    settings$.signature.set('')
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedAccount.set('acc-1')
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Hydrated reply')
    compose$.composerAttachments.set([])
    compose$.quickReplyDraftId.set('draft-1@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplyDraftThreadId.set('t-1')
    compose$.quickReplyFrom.set('me@example.com')
    compose$.quickReplySignature.set(null)
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.allocateIdentity') return { message_id: 'draft-allocated@example.com' }
            return {}
          },
        },
      },
    }
  })

  afterEach(() => {
    cancelQuickReplyDraftSave()
    cleanup()
    if (previousGo === undefined) delete (window as any).go
    else (window as any).go = previousGo
  })

  it('preserves a draft hydrated before mount and autosaves the next edit', async () => {
    renderHook(() => useQuickReply())

    expect(compose$.composer.get()).toBe('Hydrated reply')
    expect(compose$.quickReplyDraftId.get()).toBe('draft-1@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
    expect(compose$.quickReplyFrom.get()).toBe('me@example.com')

    await act(async () => {
      await Promise.resolve()
      compose$.composer.set('Hydrated reply, edited')
      await new Promise((resolve) => setTimeout(resolve, 1_300))
    })

    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true)
  })

  it('autosaves the next edit when the thread selection lands after hydration', async () => {
    ui$.selectedThread.set('')
    renderHook(() => useQuickReply())

    await act(async () => {
      compose$.composer.set('Hydrated reply')
      compose$.quickReplyDraftId.set('draft-1@example.com')
      compose$.quickReplyDraftSaved.set(true)
      compose$.quickReplyDraftThreadId.set('t-1')
      compose$.quickReplyFrom.set('me@example.com')
    })
    await act(async () => {
      ui$.selectedThread.set('t-1')
    })

    expect(compose$.quickReplyDraftId.get()).toBe('draft-1@example.com')
    await act(async () => {
      compose$.composer.set('Hydrated reply, edited')
      await new Promise((resolve) => setTimeout(resolve, 1_300))
    })

    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true)
  })

  it('autosaves the first edit after switching between equally blank thread composers', async () => {
    const nextThread = { ...thread, id: 'root-2', thread_id: 't-2', message_id: 'root-2@example.com' }
    mail$.threads.set([thread, nextThread])
    mail$.messages.set([thread, nextThread])
    compose$.composer.set('')
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyDraftThreadId.set('')
    compose$.quickReplyFrom.set('')

    renderHook(() => useQuickReply())
    await act(async () => {
      ui$.selectedThread.set('t-2')
      await Promise.resolve()
    })
    await act(async () => {
      compose$.composer.set('Reply on the new thread')
      await new Promise((resolve) => setTimeout(resolve, 1_300))
    })

    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true)
  })

  it('does not rewrite a hydrated draft merely because the hook mounted with attachments', async () => {
    compose$.composerAttachments.set([
      {
        id: 'attachment-1',
        filename: 'note.txt',
        mime: 'text/plain',
        size: 4,
        data: 'bm90ZQ==',
      },
    ])

    renderHook(() => useQuickReply())
    await act(async () => {
      await Promise.resolve()
    })

    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(false)
  })

  it('clears draft ownership when switching to a different thread', async () => {
    renderHook(() => useQuickReply())

    await act(async () => {
      ui$.selectedThread.set('t-2')
    })

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
    expect(compose$.quickReplyDraftThreadId.get()).toBe('')
    expect(compose$.quickReplyFrom.get()).toBe('')
  })
})
