import { useQuickReplyDraftFixture } from './quickReplyTestFixture'
import { describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { openDraftCompose, openDraftConversationOrCompose, openReplyInFullEditor, openComposeTab } from './compose'
import { compose$ } from './composeState'
import { saveQuickReplyDraft, withoutHydratedQuickReplyDraft } from './quickReply'
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

  it('keeps a draft written after a From change during the send', async () => {
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
    compose$.composer.set('First reply')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let releaseIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'from-change-draft@example.com' }
        identityStarted()
        await new Promise<void>((resolve) => (releaseIdentity = resolve))
        return { message_id: 'sent@example.com' }
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await allocating
    // The click emptied the box; the next reply goes in, and picking another
    // identity re-saves it against that (QuickReplyFrom).
    compose$.composer.set('Second reply')
    compose$.quickReplyFrom.set('me@example.com')
    await saveQuickReplyDraft()
    releaseIdentity()
    await send

    // That draft is the box's, not the send's: clearing the pointer would strand
    // it in Drafts.
    expect(compose$.composer.get()).toBe('Second reply')
    expect(compose$.quickReplyDraftId.get()).toBe('from-change-draft@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
  })

  it('hides the claimed draft from the click, not from the guard', async () => {
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
      message_id: 'claimed-draft@example.com',
      body: 'First reply',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, draft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('First reply')
    compose$.quickReplyDraftId.set('claimed-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let releaseIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        identityStarted()
        await new Promise<void>((resolve) => (releaseIdentity = resolve))
        return { message_id: 'sent@example.com' }
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await allocating

    // The box has let go of the draft; nothing else may put it back on screen
    // beside the reply going out.
    expect(compose$.sendingDraftIds.get()).toContain('claimed-draft@example.com')
    expect(
      withoutHydratedQuickReplyDraft(
        [thread, draft],
        compose$.quickReplyDraftId.get(),
        compose$.quickReplyDraftSaved.get(),
        compose$.sendingDraftIds.get(),
      ),
    ).toEqual([thread])

    releaseIdentity()
    await send
    // The post-send discard runs on after the send resolves.
    for (let attempt = 0; attempt < 20 && compose$.sendingDraftIds.get().length > 0; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0))
    }
    expect(compose$.sendingDraftIds.get()).toEqual([])
  })

  it('leaves a draft open in the full editor alone when the send completes', async () => {
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
    compose$.composer.set('First reply')
    compose$.quickReplyDraftId.set('claimed-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let sendStarted!: () => void
    const sending = new Promise<void>((resolve) => (sendStarted = resolve))
    let releaseSend!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send') {
        sendStarted()
        await new Promise<void>((resolve) => (releaseSend = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await sending
    // The user opens that same draft from the Drafts list while the send is out.
    openComposeTab({
      accountId: 'acc-1',
      to: 'them@example.com',
      subject: 'Re: Notification thread',
      text: 'First reply',
      draftMessageId: 'claimed-draft@example.com',
    })

    releaseSend()
    await send

    // Discarding it would empty the editor the user is typing in.
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)
  })

  it('leaves a draft alone while the user is opening it, before its tab exists', async () => {
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
    compose$.composer.set('First reply')
    compose$.quickReplyDraftId.set('claimed-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let sendStarted!: () => void
    const sending = new Promise<void>((resolve) => (sendStarted = resolve))
    let releaseSend!: () => void
    let readStarted!: () => void
    const reading = new Promise<void>((resolve) => (readStarted = resolve))
    let releaseRead!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send') {
        sendStarted()
        await new Promise<void>((resolve) => (releaseSend = resolve))
      }
      if (command === 'mail.threadRead') {
        readStarted()
        await new Promise<void>((resolve) => (releaseRead = resolve))
        return { messages: [draftRow] }
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }
    const draftRow = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-9',
      message_id: 'claimed-draft@example.com',
      body: 'First reply',
      date: 2000,
    })

    const send = sendReply()
    await sending
    // Clicking the draft in the Drafts list reads the thread first, so its tab
    // appears an RPC after the click — the window the cleanup used to fall into.
    const opening = openDraftCompose(draftRow)
    await reading

    releaseSend()
    await send
    for (let attempt = 0; attempt < 20 && compose$.sendingDraftIds.get().length > 0; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0))
    }
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)

    releaseRead()
    await opening
    const tab = compose$.tabs.get().find((item) => item.kind === 'compose')
    expect(tab?.compose?.draftMessageId).toBe('claimed-draft@example.com')
  })

  it('gives the editor its own draft when the one it opens is already being expunged', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const draftRow = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-9',
      message_id: 'claimed-draft@example.com',
      body: 'First reply',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('First reply')
    compose$.quickReplyDraftId.set('claimed-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let discardStarted!: () => void
    const discarding = new Promise<void>((resolve) => (discardStarted = resolve))
    let releaseDiscard!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.discardDraft') {
        discardStarted()
        await new Promise<void>((resolve) => (releaseDiscard = resolve))
      }
      if (command === 'mail.threadRead') return { messages: [draftRow] }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The send is done and its cleanup is on the wire when the user clicks the
    // draft: the copy they are opening is on its way out.
    await sendReply()
    await discarding
    const opening = openDraftCompose(draftRow)
    releaseDiscard()
    await opening

    // Their text opens as a draft of its own rather than one being deleted.
    const tab = compose$.tabs.get().find((item) => item.kind === 'compose')
    expect(tab?.compose?.draftMessageId).not.toBe('claimed-draft@example.com')
    expect(tab?.compose?.draftMessageId).toMatch(/^local-draft-/)
    expect(tab?.compose?.text).toBe('First reply')
  })

  it('keeps the draft held while a second open is still reading', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const draftRow = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-9',
      message_id: 'claimed-draft@example.com',
      body: 'First reply',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('First reply')
    compose$.quickReplyDraftId.set('claimed-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let sendStarted!: () => void
    const sending = new Promise<void>((resolve) => (sendStarted = resolve))
    let releaseSend!: () => void
    let reads = 0
    let secondReadStarted!: () => void
    const secondReading = new Promise<void>((resolve) => (secondReadStarted = resolve))
    let releaseSecondRead!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send') {
        sendStarted()
        await new Promise<void>((resolve) => (releaseSend = resolve))
      }
      if (command === 'mail.threadRead') {
        reads += 1
        // The first click fails; the second is still reading.
        if (reads === 1) throw new Error('read failed')
        secondReadStarted()
        await new Promise<void>((resolve) => (releaseSecondRead = resolve))
        return { messages: [draftRow] }
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await sending
    const firstOpen = openDraftCompose(draftRow)
    const secondOpen = openDraftCompose(draftRow)
    await secondReading
    await firstOpen

    releaseSend()
    await send
    for (let attempt = 0; attempt < 20 && compose$.sendingDraftIds.get().length > 0; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0))
    }
    // The failed click must not have dropped the surviving one's hold.
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)

    releaseSecondRead()
    await secondOpen
  })

  it('holds a draft row whose Message-ID has not been loaded yet', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    // Synced from its envelope: no Message-ID until the body is fetched, so the
    // quick reply identified it — and the send claimed it — by row id.
    const draftRow = message({
      id: 'draft-row-uid-42',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-9',
      message_id: '',
      body: 'First reply',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('First reply')
    compose$.quickReplyDraftId.set('draft-row-uid-42')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let sendStarted!: () => void
    const sending = new Promise<void>((resolve) => (sendStarted = resolve))
    let releaseSend!: () => void
    let readStarted!: () => void
    const reading = new Promise<void>((resolve) => (readStarted = resolve))
    let releaseRead!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send') {
        sendStarted()
        await new Promise<void>((resolve) => (releaseSend = resolve))
      }
      if (command === 'mail.threadRead') {
        readStarted()
        await new Promise<void>((resolve) => (releaseRead = resolve))
        return { messages: [draftRow] }
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await sending
    const opening = openDraftCompose(draftRow)
    await reading

    releaseSend()
    await send
    for (let attempt = 0; attempt < 20 && compose$.sendingDraftIds.get().length > 0; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0))
    }
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)

    releaseRead()
    await opening
  })

  it('hands a conversation-only open back to the quick reply', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-5',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const draftRow = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      to: 'them@example.com',
      subject: 'Re: Notification thread',
      outgoing: true,
      thread_id: 't-5',
      message_id: 'reopened-draft@example.com',
      body: 'Half-written answer',
      date: 2000,
    })
    mail$.threads.set([thread, draftRow])
    mail$.messages.set([])
    ui$.selectedThread.set('')
    compose$.composer.set('')
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') return { messages: [thread, draftRow] }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // A reply-draft opens the conversation, not the full editor: the hold that
    // protected it during the read must not outlive the hydration that takes it.
    await openDraftConversationOrCompose(draftRow)

    expect(compose$.quickReplyDraftId.get()).toBe('reopened-draft@example.com')
    expect(compose$.composer.get()).toBe('Half-written answer')
  })

  it('does not take a draft row whose Message-ID has not been synced yet', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    // Synced from its envelope: no Message-ID, and no body fetched either.
    const envelopeOnly = message({
      id: 'draft-row-uid-42',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      to: 'them@example.com',
      subject: 'Re: Notification thread',
      outgoing: true,
      thread_id: 't-1',
      message_id: '',
      body: '',
      date: 2000,
    })
    mail$.threads.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    mail$.messages.set([thread, envelopeOnly])

    // Taking it would put an empty box in front of the user calling itself their
    // draft, and give a send an id the server cannot address: a save would append
    // a second copy under it and cleanup could only ever delete one of the two.
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
    expect(compose$.quickReplyDraftThreadId.get()).toBe('')

    // The thread read back-fills the header; then it hydrates as normal.
    mail$.messages.set([thread, { ...envelopeOnly, message_id: 'now-known@example.com', body: 'Half-written answer' }])
    expect(compose$.quickReplyDraftId.get()).toBe('now-known@example.com')
    expect(compose$.composer.get()).toBe('Half-written answer')
  })

  it('saves and discards a hydrated draft under the same Message-ID it was hydrated by', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const draftRow = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      to: 'them@example.com',
      subject: 'Re: Notification thread',
      outgoing: true,
      thread_id: 't-1',
      message_id: 'hydrated-draft@example.com',
      body: 'Half-written answer',
      date: 2000,
    })
    mail$.threads.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    mail$.messages.set([thread, draftRow])
    expect(compose$.quickReplyDraftId.get()).toBe('hydrated-draft@example.com')

    // Edit it, then send: the autosave replaces that same server draft and the
    // send discards the one id, leaving nothing behind.
    compose$.composer.set('Half-written answer, now finished')
    await saveQuickReplyDraft()
    await sendReply()
    for (let attempt = 0; attempt < 20 && compose$.sendingDraftIds.get().length > 0; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0))
    }

    const saved = calls
      .filter((call) => call.command === 'mail.saveDraft')
      .map((call) => (call.payload as { draft_id?: string }).draft_id)
    const discarded = calls
      .filter((call) => call.command === 'mail.discardDraft')
      .map((call) => (call.payload as { draft_id?: string }).draft_id)
    expect(saved).toEqual(['hydrated-draft@example.com'])
    expect(discarded).toEqual(['hydrated-draft@example.com'])
  })

  it('does not overwrite a reply typed while the draft header was missing', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const envelopeOnly = message({
      id: 'draft-row-uid-42',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      message_id: '',
      body: '',
      date: 2000,
    })
    mail$.threads.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    mail$.messages.set([thread, envelopeOnly])
    expect(compose$.quickReplyDraftId.get()).toBe('')

    // The box was left free, so the user starts a reply in it.
    compose$.composer.set('A new reply, typed just now')

    // The background reread fills in that row's header and body.
    mail$.messages.set([thread, { ...envelopeOnly, message_id: 'now-known@example.com', body: 'The old saved draft' }])

    expect(compose$.composer.get()).toBe('A new reply, typed just now')
    expect(compose$.quickReplyDraftId.get()).toBe('')
  })

  it('hides the consumed draft from the conversation while the send is in flight', async () => {
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

    let sendStarted!: () => void
    const started = new Promise<void>((resolve) => (sendStarted = resolve))
    let releaseSend!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send') {
        sendStarted()
        await new Promise<void>((resolve) => (releaseSend = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await started
    // A background refresh re-delivers the still-cached draft after the quick
    // reply has let go of it.
    mail$.messages.set([thread, draft])
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
    expect(
      withoutHydratedQuickReplyDraft(
        [thread, draft],
        compose$.quickReplyDraftId.get(),
        compose$.quickReplyDraftSaved.get(),
        compose$.sendingDraftIds.get(),
      ),
    ).toEqual([thread])

    releaseSend()
    await send
  })

  it('keeps unrelated older drafts visible', () => {
    const olderDraft = message({ id: 'd0', folder_id: 'Drafts', message_id: 'draft-0@example.com' })
    const activeDraft = message({ id: 'd1', folder_id: 'Drafts', message_id: 'draft-1@example.com' })

    expect(withoutHydratedQuickReplyDraft([olderDraft, activeDraft], 'draft-1@example.com', true)).toEqual([olderDraft])
  })

  it('does not hydrate when the tail message is not a draft', () => {
    ui$.selectedThread.set('t-3')
    compose$.composer.set('unrelated text')
    const onlyMessage = message({ id: 'm1', thread_id: 't-3', folder_id: 'INBOX', date: 1000 })

    mail$.messages.set([onlyMessage])

    expect(compose$.composer.get()).toBe('unrelated text')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('clears quick reply state when the active conversation is closed', () => {
    ui$.selectedThread.set('t-4')
    compose$.composer.set('abc')
    compose$.quickReplyDraftId.set('draft-1@example.com')
    compose$.quickReplyDraftSaved.set(true)

    ui$.selectedThread.set('')

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
  })

  it('hands off the saved quick reply draft id when escalating to the full editor', async () => {
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
    const draftId = compose$.quickReplyDraftId.get()

    openReplyInFullEditor()

    expect(compose$.tabs.get()).toHaveLength(1)
    expect(compose$.tabs.get()[0].compose?.draftMessageId).toBe(draftId)
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
    expect(compose$.quickReplyDraftThreadId.get()).toBe('')
  })

  // Switched off in the quick reply (after its confirmation), it stays off in
  // the full editor: an explicit false the core honours, not unset (which it
  // reads as "answers encrypted mail, so encrypt").
  it('keeps encryption the user switched off when opening the full editor', () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      crypto: { encrypted: true },
    } as Partial<Message>)
    mail$.threads.set([thread])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('my answer')
    compose$.quickReplyEncrypt.set({ threadId: 't-1', on: false, byUser: true })

    openReplyInFullEditor()

    expect(compose$.tabs.get()[0].compose).toMatchObject({ encrypt: false, encryptOffByUser: true })
    compose$.quickReplyEncrypt.set(null)
  })

  // A draft saved encrypted (hydrated into the box) stays guarded in the full
  // editor: encrypted unless switched off on purpose, and not in localStorage.
  it('carries the guard of a hydrated encrypted draft into the full editor', () => {
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
    compose$.composer.set('> decrypted original\nmy answer')
    compose$.quickReplyEncrypt.set({ threadId: 't-1', on: true, draft: true })

    openReplyInFullEditor()

    expect(compose$.tabs.get()[0].compose).toMatchObject({ encrypt: true, encryptedDraft: true })
    compose$.quickReplyEncrypt.set(null)
  })

  // Sending to someone with a key, signed only: encryption off and the sign
  // switch on in the quick reply carry over to the full editor.
  it('carries the quick reply sign switch into the full editor', () => {
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
    compose$.composer.set('my answer')
    compose$.quickReplyEncrypt.set({ threadId: 't-1', on: false, byUser: true })
    compose$.quickReplySign.set({ threadId: 't-1', on: true })

    openReplyInFullEditor()

    expect(compose$.tabs.get()[0].compose).toMatchObject({ encrypt: false, sign: true })
    compose$.quickReplyEncrypt.set(null)
    compose$.quickReplySign.set(null)
  })

  // Another thread's choice is not this reply's.
  it('leaves the sign switch to the account default in another thread', () => {
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
    compose$.composer.set('my answer')
    compose$.quickReplySign.set({ threadId: 't-2', on: true })

    openReplyInFullEditor()

    expect(compose$.tabs.get()[0].compose?.sign).toBeUndefined()
    compose$.quickReplySign.set(null)
  })
})
