import { useQuickReplyDraftFixture } from './quickReplyTestFixture'
import { describe, expect, it, jest } from 'bun:test'
import type { Message } from '../types'
import { openComposeTab, saveComposedDraft, sendComposed } from './compose'
import { compose$ } from './composeState'
import {
  beginQuickReplyDraftRead,
  discardQuickReplyDraftIfEmpty,
  quickReplyDraftBelongsToThread,
  quickReplyDraftSaveInFlight,
  saveQuickReplyDraft,
  scheduleQuickReplyDraftSave,
  withoutHydratedQuickReplyDraft,
} from './quickReply'
import { sendReply } from './quickReplySend'
import { settings$ } from './settings'
import { ui$ } from './ui'
import { mail$ } from './mail'

const message = (overrides: Partial<Message> = {}): Message => ({
  id: 'm1',
  account_id: 'acc-notification',
  folder_id: 'inbox',
  thread_id: 't-notification',
  from_name: 'Sender',
  from_addr: 'sender@example.com',
  to: 'me@example.com',
  subject: 'Notification thread',
  preview: '',
  body: '',
  date: Math.floor(Date.parse('2026-06-11T12:00:00Z') / 1000),
  unread: true,
  starred: false,
  has_attachments: false,
  ...overrides,
})

describe('quick reply draft sharing', () => {
  const calls = useQuickReplyDraftFixture()

  it('saves the quick reply as a server draft and reuses the same id across autosaves', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Hello there')

    await saveQuickReplyDraft()

    expect(calls.filter((c) => c.command === 'mail.saveDraft')).toHaveLength(1)
    expect(calls.filter((c) => c.command === 'mail.allocateIdentity')).toHaveLength(1)
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
    const firstDraftId = compose$.quickReplyDraftId.get()
    expect(firstDraftId).toBe('draft-core@example.com')

    compose$.composer.set('Hello there, updated')
    await saveQuickReplyDraft()

    const saveCalls = calls.filter((c) => c.command === 'mail.saveDraft')
    expect(saveCalls).toHaveLength(2)
    expect(calls.filter((c) => c.command === 'mail.allocateIdentity')).toHaveLength(1)
    expect(compose$.quickReplyDraftId.get()).toBe(firstDraftId)
    expect((saveCalls[1].payload as { draft_id: string }).draft_id).toBe(firstDraftId)
  })

  it('saves the outgoing text before the debounce and never writes it to the incoming thread', async () => {
    const first = message({ account_id: 'acc-1', thread_id: 'first', message_id: 'first@example.com' })
    const second = message({ account_id: 'acc-1', thread_id: 'second', message_id: 'second@example.com' })
    mail$.threads.set([first, second])
    ui$.selectedThread.set(first.thread_id)
    compose$.composer.set('Outgoing edit')
    scheduleQuickReplyDraftSave()

    ui$.selectedThread.set(second.thread_id)
    expect(compose$.composer.peek()).toBe('')
    await quickReplyDraftSaveInFlight

    const saves = calls.filter((call) => call.command === 'mail.saveDraft')
    expect(saves).toHaveLength(1)
    expect(saves[0].payload).toMatchObject({ body: 'Outgoing edit', in_reply_to: 'first@example.com' })
    expect(compose$.quickReplyDraftId.peek()).toBe('')

    ui$.selectedThread.set(first.thread_id)
    expect(compose$.composer.peek()).toBe('Outgoing edit')
    expect(compose$.quickReplyDraftSaved.peek()).toBe(true)
  })

  it('keeps newer edits when switching while the original draft identity is still being allocated', async () => {
    const first = message({ account_id: 'acc-1', thread_id: 'first', message_id: 'first@example.com' })
    const second = message({ account_id: 'acc-1', thread_id: 'second', message_id: 'second@example.com' })
    mail$.threads.set([first, second])
    const identity = Promise.withResolvers<{ message_id: string }>()
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return identity.promise
      return {}
    }
    ui$.selectedThread.set(first.thread_id)
    compose$.composer.set('Original')
    const original = saveQuickReplyDraft()
    compose$.composer.set('Latest edit')
    scheduleQuickReplyDraftSave()
    ui$.selectedThread.set(second.thread_id)

    ui$.selectedThread.set(first.thread_id)
    expect(compose$.composer.peek()).toBe('Latest edit')
    identity.resolve({ message_id: 'same-draft@example.com' })
    await original
    await quickReplyDraftSaveInFlight

    expect(calls.filter((call) => call.command === 'mail.allocateIdentity')).toHaveLength(1)
    const saves = calls.filter((call) => call.command === 'mail.saveDraft')
    expect(saves).toHaveLength(2)
    expect(saves[1].payload).toMatchObject({ draft_id: 'same-draft@example.com', body: 'Latest edit' })
    expect(compose$.quickReplyDraftSaved.peek()).toBe(true)
  })

  it('merges restored files with newly added attachments before autosaving', async () => {
    const ancestor = message({ account_id: 'acc-1', thread_id: 'first', message_id: 'first@example.com', date: 1000 })
    const draft = message({
      ...ancestor,
      id: 'draft',
      date: 2000,
      folder_id: 'Drafts',
      message_id: 'saved-draft@example.com',
      subject: 'Re: Notification thread',
      to: ancestor.from_addr,
      from_addr: 'me@example.com',
      body: 'Saved draft',
      has_attachments: true,
      attachments: [{ filename: 'saved.txt', mime: 'text/plain', size: 1, key: 'saved-key' }],
    })
    const file = Promise.withResolvers<{ data: string; mime: string; size: number }>()
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.readAttachment') return file.promise
      return {}
    }
    mail$.threads.set([ancestor])
    ui$.selectedThread.set(ancestor.thread_id)
    mail$.messages.set([ancestor, draft])
    expect(compose$.composer.peek()).toBe('Saved draft')
    compose$.composerAttachments.push({ id: 'added', filename: 'added.txt', mime: 'text/plain', size: 1, data: 'Yg==' })

    const saving = saveQuickReplyDraft()
    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(false)
    file.resolve({ data: 'YQ==', mime: 'text/plain', size: 1 })
    await saving

    expect(compose$.composerAttachments.peek().map((attachment) => attachment.filename)).toEqual([
      'saved.txt',
      'added.txt',
    ])
    const saved = calls.find((call) => call.command === 'mail.saveDraft')
    expect(saved?.payload).toMatchObject({
      draft_id: 'saved-draft@example.com',
      attachments: [{ filename: 'saved.txt' }, { filename: 'added.txt' }],
    })
  })

  it.each(['edited', 'removed'])('uses the fresh page when a parked draft was %s elsewhere', async (change) => {
    const ancestor = message({
      account_id: 'acc-1',
      thread_id: `changed-${change}`,
      message_id: 'ancestor@example.com',
      date: 1000,
    })
    const draft = message({
      ...ancestor,
      id: 'old-draft-row',
      folder_id: 'Drafts',
      message_id: 'changed-draft@example.com',
      subject: 'Re: Notification thread',
      to: ancestor.from_addr,
      from_addr: 'me@example.com',
      body: 'Older reply',
      date: 2000,
    })
    mail$.threads.set([ancestor])
    ui$.selectedThread.set(ancestor.thread_id)
    mail$.messages.set([ancestor, draft])
    ui$.selectedThread.set('another-conversation')
    ui$.selectedThread.set(ancestor.thread_id)
    expect(compose$.composer.peek()).toBe('Older reply')

    const fresh = change === 'edited' ? [ancestor, { ...draft, id: 'new-draft-row', body: 'Newer reply' }] : [ancestor]
    const finishRead = beginQuickReplyDraftRead(ancestor.thread_id)
    expect(finishRead).toBeDefined()
    await finishRead?.(fresh)
    mail$.messages.set(fresh)
    expect(compose$.composer.peek()).toBe(change === 'edited' ? 'Newer reply' : '')
  })

  it('keeps draft ownership when a fresh page first shows its own saved reply', async () => {
    const ancestor = message({
      account_id: 'acc-1',
      thread_id: 'first-observation',
      message_id: 'ancestor@example.com',
      date: 1000,
    })
    mail$.threads.set([ancestor])
    mail$.messages.set([ancestor])
    ui$.selectedThread.set(ancestor.thread_id)
    compose$.composer.set('Our reply')
    scheduleQuickReplyDraftSave()
    ui$.selectedThread.set('another-conversation')
    await quickReplyDraftSaveInFlight
    ui$.selectedThread.set(ancestor.thread_id)

    const finishRead = beginQuickReplyDraftRead(ancestor.thread_id)
    expect(finishRead).toBeDefined()
    compose$.composer.set('Our reply, edited since returning')
    await finishRead?.([
      ancestor,
      message({
        ...ancestor,
        id: 'saved-row',
        folder_id: 'Drafts',
        message_id: 'draft-core@example.com',
        subject: 'Re: Notification thread',
        to: ancestor.from_addr,
        from_addr: 'me@example.com',
        body: 'Our reply\r\n',
        date: 2000,
      }),
    ])

    expect(compose$.composer.peek()).toBe('Our reply, edited since returning')
    expect(compose$.quickReplyDraftId.peek()).toBe('draft-core@example.com')
  })

  it('stops comparing an older restored reply after the editor saves new text', async () => {
    const ancestor = message({
      account_id: 'acc-1',
      thread_id: 'new-save-after-restore',
      message_id: 'ancestor@example.com',
    })
    mail$.threads.set([ancestor])
    mail$.messages.set([ancestor])
    ui$.selectedThread.set(ancestor.thread_id)
    compose$.composer.set('Initial reply')
    scheduleQuickReplyDraftSave()
    ui$.selectedThread.set('another-conversation')
    await quickReplyDraftSaveInFlight
    ui$.selectedThread.set(ancestor.thread_id)

    const finishRead = beginQuickReplyDraftRead(ancestor.thread_id)
    expect(finishRead).toBeDefined()
    await finishRead?.([ancestor])
    expect(compose$.composer.peek()).toBe('Initial reply')

    compose$.composer.set('Updated reply')
    await saveQuickReplyDraft()
    expect(beginQuickReplyDraftRead(ancestor.thread_id)).toBeUndefined()
    expect(compose$.quickReplyDraftId.peek()).toBe('draft-core@example.com')
  })

  it('waits for draft validation that starts during the attachment read', async () => {
    const ancestor = message({
      account_id: 'acc-1',
      thread_id: 'overlapping-draft-reads',
      message_id: 'ancestor@example.com',
      date: 1000,
    })
    const draft = message({
      ...ancestor,
      id: 'saved-row',
      folder_id: 'Drafts',
      message_id: 'attached-draft@example.com',
      subject: 'Re: Notification thread',
      to: ancestor.from_addr,
      from_addr: 'me@example.com',
      body: 'Reply with a file',
      date: 2000,
      has_attachments: true,
      attachments: [{ filename: 'saved.txt', mime: 'text/plain', size: 1, key: 'saved-key' }],
    })
    const file = Promise.withResolvers<{ data: string; mime: string; size: number }>()
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.readAttachment') return file.promise
      return {}
    }
    mail$.threads.set([ancestor])
    ui$.selectedThread.set(ancestor.thread_id)
    mail$.messages.set([ancestor, draft])
    ui$.selectedThread.set('another-conversation')
    ui$.selectedThread.set(ancestor.thread_id)
    compose$.composer.set('Edited reply with a file')

    const saving = saveQuickReplyDraft()
    const finishRead = beginQuickReplyDraftRead(ancestor.thread_id)
    expect(finishRead).toBeDefined()
    file.resolve({ data: 'YQ==', mime: 'text/plain', size: 1 })
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(false)

    await finishRead?.([ancestor, draft])
    await saving
    expect(calls.find((call) => call.command === 'mail.saveDraft')?.payload).toMatchObject({
      draft_id: 'attached-draft@example.com',
      body: 'Edited reply with a file',
      attachments: [{ filename: 'saved.txt' }],
    })
  })

  it('marks a newly minted draft unsaved when the hydrated draft is claimed by an editor', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Updated reply')
    compose$.quickReplyDraftId.set('hydrated-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplyDraftThreadId.set('t-1')
    openComposeTab({
      accountId: 'acc-1',
      draftMessageId: 'hydrated-draft@example.com',
      title: 'Claimed draft',
    })

    let saveStarted!: () => void
    const started = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'replacement-draft@example.com' }
      if (command === 'mail.saveDraft') {
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      return {}
    }

    const saving = saveQuickReplyDraft()
    await started

    expect(compose$.quickReplyDraftId.get()).not.toBe('hydrated-draft@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)

    releaseSave()
    await saving
    expect(compose$.quickReplyDraftId.get()).toBe('replacement-draft@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
  })

  it('does nothing when there is no active thread or the composer is empty', async () => {
    await saveQuickReplyDraft()
    expect(calls.some((c) => c.command === 'mail.saveDraft')).toBe(false)
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('discards the quick reply draft once cleared back to blank', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    const draft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      from_addr: 'me@example.com',
      message_id: 'draft-1@example.com',
      body: 'Hello there',
      date: thread.date + 1,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Hello there')
    compose$.quickReplyDraftId.set('draft-1@example.com')
    compose$.quickReplyDraftSaved.set(true)

    compose$.composer.set('')
    await discardQuickReplyDraftIfEmpty()

    expect(calls.find((c) => c.command === 'mail.discardDraft')?.payload).toMatchObject({
      account_id: 'acc-1',
      draft_id: 'draft-1@example.com',
      thread_id: 't-1',
    })
    expect(mail$.messages.get()).toEqual([thread])
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('discards the copy a failed autosave may have left once cleared back to blank', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Hello there')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'uncertain@example.com' }
      // The host gave up waiting; the core may still have written the draft.
      if (command === 'mail.saveDraft') throw new Error('sidecar save_draft timeout')
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await saveQuickReplyDraft()
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)

    compose$.composer.set('')
    await discardQuickReplyDraftIfEmpty()

    expect(calls.find((c) => c.command === 'mail.discardDraft')?.payload).toMatchObject({
      account_id: 'acc-1',
      draft_id: 'uncertain@example.com',
    })
    expect(compose$.quickReplyDraftId.get()).toBe('')
  })

  it('discards the draft when the composer is cleared while the autosave is still in flight', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('abc')

    let saveStarted!: () => void
    const started = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'draft-core@example.com' }
      if (command === 'mail.saveDraft') {
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      return {}
    }

    const save = saveQuickReplyDraft()
    await started

    // Cleared mid-save: quickReplyDraftSaved is still false here, so this
    // discard alone can't see the draft the RPC is about to create.
    compose$.composer.set('')
    const discard = discardQuickReplyDraftIfEmpty()
    releaseSave()
    await save
    await discard

    expect(calls.filter((c) => c.command === 'mail.saveDraft')).toHaveLength(1)
    const discardCalls = calls.filter((c) => c.command === 'mail.discardDraft')
    expect(discardCalls).toHaveLength(1)
    expect(discardCalls[0].payload).toMatchObject({ account_id: 'acc-1', thread_id: 't-1' })
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('does nothing when there is no saved draft to discard', async () => {
    await discardQuickReplyDraftIfEmpty()
    expect(calls.some((c) => c.command === 'mail.discardDraft')).toBe(false)
  })

  it('hydrates the quick reply from a saved draft at the tail of the conversation', () => {
    ui$.selectedThread.set('t-2')
    const ancestor = message({
      id: 'm1',
      thread_id: 't-2',
      folder_id: 'INBOX',
      date: 1000,
    })
    const draft = message({
      id: 'd1',
      thread_id: 't-2',
      folder_id: 'Drafts',
      from_addr: 'me@example.com',
      message_id: 'draft-1@example.com',
      body: 'saved draft body',
      date: 2000,
    })

    mail$.messages.set([ancestor, draft])

    expect(compose$.composer.get()).toBe('saved draft body')
    expect(compose$.quickReplyDraftId.get()).toBe('draft-1@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
    expect(compose$.quickReplyFrom.get()).toBe('me@example.com')
  })

  it.each([
    ['is missing', { body_missing: true }],
    ['could not be decrypted', { crypto: { encrypted: true, problem: 'no_key' } }],
  ])('leaves the quick reply empty and unbound when the tail draft body %s', (_, unreadable) => {
    ui$.selectedThread.set('t-2')
    const ancestor = message({ id: 'm1', thread_id: 't-2', folder_id: 'INBOX', date: 1000 })
    const draft = message({
      id: 'd1',
      thread_id: 't-2',
      folder_id: 'Drafts',
      from_addr: 'me@example.com',
      message_id: 'draft-1@example.com',
      body: '',
      date: 2000,
      ...(unreadable as Partial<Message>),
    })

    mail$.messages.set([ancestor, draft])

    expect(compose$.quickReplyDraftId.get()).toBe('')
  })

  // A draft saved encrypted in a plain conversation must not come back as a
  // plain reply (the conversation's default), nor be switched off by itself.
  it('hydrates a draft saved encrypted as an encrypted, guarded quick reply', () => {
    compose$.quickReplyEncrypt.set(null)
    ui$.selectedThread.set('t-2')
    const ancestor = message({ id: 'm1', thread_id: 't-2', folder_id: 'INBOX', date: 1000 })
    const draft = message({
      id: 'd1',
      thread_id: 't-2',
      folder_id: 'Drafts',
      from_addr: 'me@example.com',
      message_id: 'draft-1@example.com',
      body: 'saved encrypted body',
      date: 2000,
      crypto: { encrypted: true },
    } as Partial<Message>)

    mail$.messages.set([ancestor, draft])

    expect(compose$.composer.get()).toBe('saved encrypted body')
    expect(compose$.quickReplyEncrypt.get()).toEqual({ threadId: 't-2', on: true, draft: true })
    compose$.quickReplyEncrypt.set(null)
  })

  it.each([
    ['a rich body', { body_html: '<p>rich</p>' }],
    ['a Bcc', { bcc: 'hidden@example.com' }],
    ['a Reply-To', { reply_to: 'elsewhere@example.com' }],
    ['other recipients', { to: 'someone-else@example.com' }],
    ['added recipients', { cc: 'added@example.com' }],
    ['another subject', { subject: 'Re: Reworded' }],
  ])('leaves a draft with %s to the full composer', (_, draft) => {
    const thread = message({ id: 'm1', thread_id: 't-2', folder_id: 'INBOX', date: 1000 })
    mail$.threads.set([thread])
    ui$.selectedThread.set('t-2')

    mail$.messages.set([
      thread,
      message({
        id: 'd1',
        thread_id: 't-2',
        folder_id: 'Drafts',
        to: 'Sender <sender@example.com>',
        subject: 'Re: Notification thread',
        message_id: 'draft-1@example.com',
        body: 'saved draft body',
        date: 2000,
        ...draft,
      }),
    ])

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftId.get()).toBe('')
  })

  it('takes a plain draft the quick reply could have written itself', () => {
    const thread = message({ id: 'm1', thread_id: 't-2', folder_id: 'INBOX', date: 1000 })
    mail$.threads.set([thread])
    ui$.selectedThread.set('t-2')

    mail$.messages.set([
      thread,
      message({
        id: 'd1',
        thread_id: 't-2',
        folder_id: 'Drafts',
        to: 'sender@example.com',
        subject: 'Re: Notification thread',
        message_id: 'draft-1@example.com',
        body: 'saved draft body',
        date: 2000,
      }),
    ])

    expect(compose$.composer.get()).toBe('saved draft body')
  })

  it('recognizes a draft hydrated before the quick-reply component mounts', () => {
    compose$.composer.set('Already hydrated')
    compose$.quickReplyDraftId.set('draft-1@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplyDraftThreadId.set('t-1')

    expect(quickReplyDraftBelongsToThread('t-1')).toBe(true)
    expect(quickReplyDraftBelongsToThread('t-2')).toBe(false)
    compose$.quickReplyDraftSaved.set(false)
    expect(quickReplyDraftBelongsToThread('t-1')).toBe(true)
  })

  it('keeps the hydrated draft hidden after an optimistic sent bubble is appended', () => {
    const ancestor = message({ id: 'm1', folder_id: 'INBOX', message_id: 'root@example.com' })
    const draft = message({
      id: 'd1',
      folder_id: 'Drafts',
      message_id: 'draft-1@example.com',
    })
    const sending = message({ id: 'local-send-1', folder_id: 'INBOX', send_status: 'sending' })

    expect(withoutHydratedQuickReplyDraft([ancestor, draft, sending], 'draft-1@example.com', true)).toEqual([
      ancestor,
      sending,
    ])
  })

  it('does not rehydrate the consumed draft when the thread refreshes during send', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const draft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      from_addr: 'me@example.com',
      message_id: 'reply-draft@example.com',
      body: 'Hello there',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Hello there')
    compose$.quickReplyDraftId.set('reply-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send') {
        // A mail.synced refresh can land before sendReply gets to discard the
        // server draft, temporarily making that draft the conversation tail.
        mail$.messages.set([thread, draft])
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await sendReply()

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('gives the sent bubble of an encrypted reply our own verified lock at once', async () => {
    // The stored Sent copy gets it once cached; until the thread reloads the
    // bubble is all the user sees of the send.
    for (const on of [true, false]) {
      const thread = message({
        id: 'root',
        account_id: 'acc-1',
        thread_id: 't-1',
        folder_id: 'INBOX',
        from_addr: 'them@example.com',
        message_id: 'root@example.com',
        date: 1000,
      })
      mail$.threads.set([thread])
      mail$.messages.set([thread])
      ui$.selectedThread.set('t-1')
      compose$.composer.set('Secret reply')
      compose$.quickReplyEncrypt.set({ threadId: 't-1', on })
      compose$.quickReplySignatureOff.set(false)
      settings$.signature.set('')
      ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
        calls.push({ command, payload })
        if (command === 'mail.allocateIdentity') return { message_id: `sent-${on}@example.com` }
        if (command === 'mail.folderList') return { folders: [] }
        if (command === 'mail.threadList') return { threads: [] }
        return {}
      }

      await sendReply()

      const bubble = mail$.messages.get().find((m) => m.body === 'Secret reply')
      expect(bubble).toBeDefined()
      // The quick reply always states its choice, so the core never has to
      // guess.
      const sent = [...calls].reverse().find((c) => c.command === 'mail.send')?.payload as {
        encrypt: boolean
        plain: boolean
      }
      expect(sent).toMatchObject({ encrypt: on, plain: !on })
      if (on) {
        expect(bubble?.crypto).toMatchObject({ encrypted: true, signed: true, own: true, trust: 'verified' })
        expect(bubble?.crypto?.problem ?? null).toBeNull()
      } else {
        expect(bubble?.crypto ?? null).toBeNull()
      }
      compose$.quickReplyEncrypt.set(null)
    }
  })

  // Signed only, answering encrypted mail: the box is cleared during the send,
  // which must not bring back the default (encrypt, as the original was).
  it('sends a signed-only reply to encrypted mail as chosen', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
      crypto: { encrypted: true },
    } as Partial<Message>)
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Signed reply')
    compose$.quickReplyEncrypt.set({ threadId: 't-1', on: false, byUser: true })
    compose$.quickReplySign.set({ threadId: 't-1', on: true })
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent-signed@example.com' }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await sendReply()

    const sent = [...calls].reverse().find((c) => c.command === 'mail.send')?.payload
    expect(sent).toMatchObject({ encrypt: false, plain: true, sign: true })
    // The next reply in the thread starts from the defaults again.
    expect(compose$.quickReplyEncrypt.get()).toBeNull()
    expect(compose$.quickReplySign.get()).toBeNull()
  })

  it('says plain to the core only when encryption was switched off, not when the flag is missing', async () => {
    // A reply to encrypted mail that says neither is encrypted by the
    // core, so a path that loses `encrypt` can't downgrade it by accident.
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'd@example.com' }
      return {}
    }
    const base = { accountId: 'acc-1', to: 'x@example.com', subject: 's', rich: false, content: 'hi', attachments: [] }
    const last = (command: string) =>
      [...calls].reverse().find((c) => c.command === command)?.payload as { encrypt: boolean; plain: boolean }
    for (const [encrypt, plain] of [
      [true, false],
      [false, true],
      [undefined, false],
    ] as const) {
      await sendComposed({ ...base, encrypt })
      expect(last('mail.send')).toMatchObject({ encrypt: !!encrypt, plain })
      await saveComposedDraft({ ...base, draftMessageId: 'd@example.com', encrypt })
      expect(last('mail.saveDraft')).toMatchObject({ encrypt: !!encrypt, plain })
    }
  })

  it('sends and saves a body holding decrypted text encrypted, even with encryption off', async () => {
    // An unfolded quote of decrypted mail is in the body itself: no path may
    // send or save it in the clear.
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'd@example.com' }
      return {}
    }
    const base = { accountId: 'acc-1', to: 'x@example.com', subject: 's', rich: false, content: 'hi', attachments: [] }
    const last = (command: string) =>
      [...calls].reverse().find((c) => c.command === command)?.payload as { encrypt: boolean; plain: boolean }
    await sendComposed({ ...base, encrypt: false, decryptedInBody: true })
    expect(last('mail.send')).toMatchObject({ encrypt: true, plain: false })
    await saveComposedDraft({ ...base, draftMessageId: 'd@example.com', encrypt: false, decryptedInBody: true })
    expect(last('mail.saveDraft')).toMatchObject({ encrypt: true, plain: false })
  })

  it('says who an encrypted send did not reach, as an error rather than "sent"', async () => {
    // Encrypted mail goes out as one copy per Bcc; the core reports a refused
    // copy instead of failing a send others already received.
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Reply')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send') return { ok: true, undelivered: ['blind@example.com'] }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }
    const base = { accountId: 'acc-1', to: 'x@example.com', subject: 's', rich: false, content: 'hi', attachments: [] }
    expect(await sendComposed(base)).toEqual(['blind@example.com'])

    await sendReply()
    expect(ui$.toastTone.get()).toBe('error')
    expect(ui$.toast.get()).toContain('blind@example.com')
  })

  it('discards a matching loaded draft when its quick-reply ownership was lost', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
      has_draft: true,
    })
    const draft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      from_addr: 'me@example.com',
      message_id: 'reply-draft@example.com',
      body: 'Reply that was sent',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Reply that was sent')
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyDraftThreadId.set('')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    await sendReply()
    await new Promise((resolve) => setTimeout(resolve, 0))

    expect(calls.find((call) => call.command === 'mail.discardDraft')?.payload).toMatchObject({
      account_id: 'acc-1',
      draft_id: 'reply-draft@example.com',
      thread_id: 't-1',
    })
    expect(mail$.messages.get().some((item) => item.id === 'draft-row')).toBe(false)
    expect(mail$.threads.get().find((item) => item.thread_id === 't-1')?.has_draft).toBe(false)
  })

  it('keeps the consumed draft guarded while the post-send discard is still in flight', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const draft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      message_id: 'reply-draft@example.com',
      body: 'Consumed reply',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Consumed reply')
    compose$.quickReplyDraftId.set('reply-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let releaseDiscard!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.discardDraft') {
        await new Promise<void>((resolve) => (releaseDiscard = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await sendReply()

    // SMTP has returned, so the pending payload is gone and a refresh can
    // already have swapped the optimistic bubble for the canonical Sent copy —
    // while the server draft this reply consumed is still there, and still the
    // conversation tail. The guard has to outlive its bubble until the discard
    // resolves, or the just-sent text lands back in the cleared composer.
    const sentCopy = message({
      id: 'sent-row',
      account_id: 'acc-1',
      folder_id: 'Sent',
      thread_id: 't-1',
      message_id: 'sent@example.com',
      date: 1500,
    })
    mail$.messages.set([thread, sentCopy, draft])

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftId.get()).toBe('')

    releaseDiscard()
    await new Promise((resolve) => setTimeout(resolve, 0))

    expect(mail$.messages.get().some((message) => message.id === 'draft-row')).toBe(false)
  })

  it("keeps a sent reply's draft suppressed and retries when its discard fails", async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const other = message({ id: 'other', account_id: 'acc-1', thread_id: 't-2', message_id: 'other@example.com' })
    const draft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      message_id: 'reply-draft@example.com',
      body: 'Consumed reply',
      date: 2000,
    })
    mail$.threads.set([thread, other])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Consumed reply')
    compose$.quickReplyDraftId.set('reply-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let discardFails = true
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.discardDraft' && discardFails) throw new Error('offline')
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }
    const flush = async () => {
      for (let i = 0; i < 50; i++) await Promise.resolve()
    }

    jest.useFakeTimers()
    try {
      await sendReply()
      await flush()

      // The message went out, so the copy left behind is stale rather than a
      // safety net. Coming back to the thread before the Sent copy is cached
      // leaves the draft as its tail — it must not hydrate the box with text
      // that was already sent.
      ui$.selectedThread.set('t-2')
      ui$.selectedThread.set('t-1')
      mail$.messages.set([thread, draft])
      expect(compose$.composer.get()).toBe('')
      expect(compose$.sendingDraftIds.get()).toContain('reply-draft@example.com')

      discardFails = false
      jest.advanceTimersByTime(5_000)
      await flush()

      expect(calls.filter((call) => call.command === 'mail.discardDraft')).toHaveLength(2)
      expect(compose$.sendingDraftIds.get()).not.toContain('reply-draft@example.com')
      expect(mail$.messages.get().some((message) => message.id === 'draft-row')).toBe(false)
    } finally {
      jest.useRealTimers()
    }
  })
})
