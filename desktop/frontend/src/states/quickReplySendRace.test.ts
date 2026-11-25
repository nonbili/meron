import { useQuickReplyDraftFixture } from './quickReplyTestFixture'
import { describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { activateConversationTab, openReplyInFullEditor } from './compose'
import { compose$ } from './composeState'
import { quickReplyDraftBelongsToThread, saveQuickReplyDraft, scheduleQuickReplyDraftSave } from './quickReply'
import { retrySend, sendReply } from './quickReplySend'
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

  it('keeps the id a failed autosave was writing under', async () => {
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

    let saveFails = true
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'draft-core@example.com' }
      // The host gave up waiting; the core may still have written the draft.
      if (command === 'mail.saveDraft' && saveFails) throw new Error('sidecar save_draft timeout')
      return {}
    }

    await saveQuickReplyDraft()
    expect(compose$.quickReplyDraftId.get()).toBe('draft-core@example.com')
    // Kept for reuse, but not known to be on the server.
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)

    saveFails = false
    compose$.composer.set('Hello there, again')
    await saveQuickReplyDraft()

    const saves = calls.filter((call) => call.command === 'mail.saveDraft')
    expect(calls.filter((call) => call.command === 'mail.allocateIdentity')).toHaveLength(1)
    expect((saves[1].payload as { draft_id: string }).draft_id).toBe('draft-core@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
  })

  it('allows a different draft to hydrate after the sent draft cleanup settles', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const sentDraft = message({
      id: 'sent-draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      to: 'them@example.com',
      subject: 'Re: Notification thread',
      outgoing: true,
      thread_id: 't-1',
      message_id: 'sent-draft@example.com',
      body: 'First reply',
      date: 2000,
    })
    mail$.threads.set([thread])
    mail$.messages.set([thread, sentDraft])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('First reply')
    compose$.quickReplyDraftId.set('sent-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    await sendReply()
    await new Promise((resolve) => setTimeout(resolve, 0))

    const newerDraft = message({
      id: 'new-draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      to: 'them@example.com',
      subject: 'Re: Notification thread',
      outgoing: true,
      thread_id: 't-1',
      message_id: 'different-draft@example.com',
      body: 'Written elsewhere',
      date: 3000,
    })
    mail$.messages.set([thread, newerDraft])

    expect(compose$.composer.get()).toBe('Written elsewhere')
    expect(compose$.quickReplyDraftId.get()).toBe('different-draft@example.com')
  })

  it('keeps the consumed draft guarded across a navigation round trip during send', async () => {
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
    ui$.selectedThread.set('t-2')
    ui$.selectedThread.set('t-1')
    mail$.messages.set([thread, draft])

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)

    releaseSend()
    await send
  })

  it('discards the draft an autosave was still writing when the send overtook it', async () => {
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
    compose$.composer.set('Racing the autosave')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let saveStarted!: () => void
    const started = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        return { message_id: (payload as { draft?: boolean }).draft ? 'raced-draft@example.com' : 'sent@example.com' }
      }
      if (command === 'mail.saveDraft') {
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The autosave is on the wire — its draft exists on the server, but the
    // "saved" flag hasn't flipped yet — when the user hits send.
    const save = saveQuickReplyDraft()
    await started
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
    const send = sendReply()
    await new Promise((resolve) => setTimeout(resolve, 0))
    // The discard waits for that save rather than deleting nothing ahead of it.
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)
    // The user starts the next reply straight away, so the save's own
    // "composer went blank under me" cleanup won't fire: the send is the only
    // thing left that still knows this draft id.
    compose$.composer.set('Next reply')

    releaseSave()
    await save
    await send

    const discard = calls.find((call) => call.command === 'mail.discardDraft')
    expect((discard?.payload as { draft_id?: string })?.draft_id).toBe('raced-draft@example.com')
  })

  it('keeps the saved draft when a send that overtook its autosave fails', async () => {
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
    compose$.composer.set('Reply text')
    compose$.quickReplyDraftId.set('safety-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let saveStarted!: () => void
    const started = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.saveDraft') {
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      if (command === 'mail.send') throw new Error('smtp unreachable')
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // A replacement autosave of the already-saved draft is on the wire when the
    // send takes the box, and the send then fails.
    const save = saveQuickReplyDraft()
    await started
    const send = sendReply()
    await new Promise((resolve) => setTimeout(resolve, 0))
    releaseSave()
    await save
    await send

    // The draft is the only copy of a reply that never went out.
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)
  })

  it('does not hand a draft written after the send was clicked to that send', async () => {
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
    const started = new Promise<void>((resolve) => (identityStarted = resolve))
    let releaseIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'later-draft@example.com' }
        identityStarted()
        await new Promise<void>((resolve) => (releaseIdentity = resolve))
        return { message_id: 'sent@example.com' }
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The send is still allocating its identity — an await the user can type
    // through — when the next autosave writes a draft the send never took.
    const send = sendReply()
    await started
    compose$.composer.set('First reply, and more written after the click')
    await saveQuickReplyDraft()
    releaseIdentity()
    await send

    const discarded = calls
      .filter((call) => call.command === 'mail.discardDraft')
      .map((call) => (call.payload as { draft_id?: string }).draft_id)
    expect(discarded).not.toContain('later-draft@example.com')
  })

  it('discards the draft an autosave lands after the full editor took the reply over', async () => {
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
    compose$.composer.set('Escalate me')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let saveStarted!: () => void
    const started = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'orphan-draft@example.com' }
      if (command === 'mail.saveDraft') {
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The first autosave is still allocating its id — nothing the escalation can
    // hand over — so the full editor starts a draft of its own.
    const save = saveQuickReplyDraft()
    await started
    openReplyInFullEditor()
    // Back in the conversation, writing the next reply: the box being non-blank
    // is what used to leave the landed copy stranded in Drafts.
    activateConversationTab()
    compose$.composer.set('A different reply')

    releaseSave()
    await save

    const discard = calls.find((call) => call.command === 'mail.discardDraft')
    expect((discard?.payload as { draft_id?: string })?.draft_id).toBe('orphan-draft@example.com')
  })

  it('does not discard the reclaimed draft when a failed send is retried', async () => {
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
    mail$.threads.set([thread, other])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Reply text')
    compose$.quickReplyDraftId.set('safety-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let failSend = true
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.send' && failSend) throw new Error('smtp unreachable')
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await sendReply()
    const failed = mail$.messages.get().find((item) => item.send_status === 'failed')
    expect(failed).toBeTruthy()

    // Leaving the thread releases the failed send's safety draft, and coming
    // back hydrates it into the box, where the user edits it further.
    ui$.selectedThread.set('t-2')
    ui$.selectedThread.set('t-1')
    const safetyDraft = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      to: 'them@example.com',
      subject: 'Re: Notification thread',
      outgoing: true,
      thread_id: 't-1',
      message_id: 'safety-draft@example.com',
      body: 'Reply text',
      date: 2000,
    })
    mail$.messages.set([thread, safetyDraft])
    expect(compose$.quickReplyDraftId.get()).toBe('safety-draft@example.com')

    failSend = false
    calls.length = 0
    await retrySend(failed!.id)

    // The box is still editing that draft — the retry's cleanup isn't its owner.
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)
    expect(compose$.quickReplyDraftId.get()).toBe('safety-draft@example.com')
  })

  it('leaves a draft the full editor took over out of the quick reply', () => {
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
    compose$.composer.set('Escalate me')
    compose$.quickReplyDraftId.set('escalated-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    openReplyInFullEditor()
    activateConversationTab()

    // A refresh delivers the escalated draft as the thread's tail.
    const escalated = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      message_id: 'escalated-draft@example.com',
      body: 'Escalate me',
      date: 2000,
    })
    mail$.messages.set([thread, escalated])

    // Two editors on one server draft is how it gets discarded under the tab.
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.composer.get()).not.toBe('Escalate me')
  })

  it('discards a draft that lands between the send click and its guard', async () => {
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
    compose$.composer.set('Racing reply')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let saveStarted!: () => void
    const saveRunning = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let releaseIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'inflight-draft@example.com' }
        identityStarted()
        await new Promise<void>((resolve) => (releaseIdentity = resolve))
        return { message_id: 'sent@example.com' }
      }
      if (command === 'mail.saveDraft') {
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The autosave is on the wire at the click, and lands while the send is
    // still allocating its identity — before the send has a guard to adopt it.
    const save = saveQuickReplyDraft()
    await saveRunning
    const send = sendReply()
    await allocating
    releaseSave()
    await save
    // The late save belongs to the send now, not to the box. A remount in this
    // allocation window must not preserve the id the send will discard.
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftThreadId.get()).toBe('')
    expect(quickReplyDraftBelongsToThread('t-1')).toBe(false)
    releaseIdentity()
    await send

    const discard = calls.find((call) => call.command === 'mail.discardDraft')
    expect((discard?.payload as { draft_id?: string })?.draft_id).toBe('inflight-draft@example.com')
  })

  it('does not hydrate a draft into the box while a send is still allocating its identity', async () => {
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
    mail$.threads.set([thread, other])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Reply text')
    compose$.quickReplyDraftId.set('claimed-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplyDraftThreadId.set('t-1')
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
    // The user closes the conversation and reopens it while the allocation
    // stalls — which resets the box — then a refresh delivers the claimed draft
    // as the thread's tail.
    ui$.selectedThread.set('')
    ui$.selectedThread.set('t-1')
    const claimed = message({
      id: 'draft-row',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 't-1',
      message_id: 'claimed-draft@example.com',
      body: 'Reply text',
      date: 2000,
    })
    mail$.messages.set([thread, claimed])

    // The send owns it and will discard it; editing it here would lose that work.
    expect(compose$.quickReplyDraftId.get()).toBe('')

    releaseIdentity()
    await send
    const discarded = calls
      .filter((call) => call.command === 'mail.discardDraft')
      .map((call) => (call.payload as { draft_id?: string }).draft_id)
    expect(discarded).toContain('claimed-draft@example.com')
  })

  it('hands the draft back to the box when the send never gets off the ground', async () => {
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
    compose$.composer.set('Racing reply')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let saveStarted!: () => void
    const saveRunning = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'inflight-draft@example.com' }
        identityStarted()
        await new Promise<void>((_resolve, reject) => (failIdentity = () => reject(new Error('offline'))))
      }
      if (command === 'mail.saveDraft') {
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The autosave lands mid-send and hands its id to the claim; then the send
    // dies before it ever has a guard.
    const save = saveQuickReplyDraft()
    await saveRunning
    const send = sendReply().catch(() => undefined)
    await allocating
    releaseSave()
    await save
    failIdentity()
    await send

    // The box is left holding a draft it knows is on the server.
    expect(compose$.quickReplyDraftId.get()).toBe('inflight-draft@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)
  })

  it('keeps the unsent draft when a send dies after the user has left the thread', async () => {
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
    compose$.composer.set('Never sent')
    compose$.quickReplyDraftId.set('unsent-draft@example.com')
    compose$.quickReplyDraftSaved.set(true)
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        identityStarted()
        await new Promise<void>((_resolve, reject) => (failIdentity = () => reject(new Error('offline'))))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply().catch(() => undefined)
    await allocating
    ui$.selectedThread.set('')
    failIdentity()
    await send

    // Nothing was ever sent, so the draft is still the only copy of that reply.
    expect(calls.some((call) => call.command === 'mail.discardDraft' || call.command === 'mail.delete')).toBe(false)
  })

  it('leaves a reply written during the send alone instead of clearing the box', async () => {
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
        if ((payload as { draft?: boolean }).draft) return { message_id: 'later-draft@example.com' }
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
    compose$.composer.set('A second reply, written while the first was going out')
    await saveQuickReplyDraft()
    releaseIdentity()
    await send

    // The send took the first reply; the second is the user's, and so is the
    // draft holding it.
    expect(compose$.composer.get()).toBe('A second reply, written while the first was going out')
    expect(compose$.quickReplyDraftId.get()).toBe('later-draft@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
  })

  it("does not clear another thread's quick reply when a send completes", async () => {
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
    mail$.threads.set([thread, other])
    mail$.messages.set([thread, other])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Reply in the first thread')
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
    ui$.selectedThread.set('t-2')
    compose$.composer.set('Half-written reply in the second thread')
    releaseIdentity()
    await send

    expect(compose$.composer.get()).toBe('Half-written reply in the second thread')
  })

  it('gives a reply written during the send a draft of its own', async () => {
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

    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let releaseIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'later-draft@example.com' }
        identityStarted()
        await new Promise<void>((resolve) => (releaseIdentity = resolve))
        return { message_id: 'sent@example.com' }
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The send took the box's saved draft with it; the reply written next must
    // not be saved over that same server copy.
    const send = sendReply()
    await allocating
    compose$.composer.set('A second reply')
    await saveQuickReplyDraft()
    releaseIdentity()
    await send

    const saved = calls
      .filter((call) => call.command === 'mail.saveDraft')
      .map((call) => (call.payload as { draft_id?: string }).draft_id)
    expect(saved).toEqual(['later-draft@example.com'])
    const discarded = calls
      .filter((call) => call.command === 'mail.discardDraft')
      .map((call) => (call.payload as { draft_id?: string }).draft_id)
    expect(discarded).toEqual(['claimed-draft@example.com'])
    expect(compose$.quickReplyDraftId.get()).toBe('later-draft@example.com')
    expect(compose$.quickReplyDraftSaved.get()).toBe(true)
  })

  it('keeps an attachment swapped in during the send', async () => {
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
    compose$.composer.set('With a file')
    compose$.composerAttachments.set([
      { id: 'a1', filename: 'first.pdf', mime: 'application/pdf', size: 10, data: 'AA==' },
    ])
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

    // Same text, same attachment count, different file: by value the box looks
    // untouched, but the user picked this one after clicking send.
    const send = sendReply()
    await allocating
    compose$.composerAttachments.set([
      { id: 'a2', filename: 'second.pdf', mime: 'application/pdf', size: 12, data: 'BB==' },
    ])
    releaseIdentity()
    await send

    expect(compose$.composerAttachments.get().map((item) => item.filename)).toEqual(['second.pdf'])
  })

  it('cancels the pending autosave the moment the send takes the box', async () => {
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
    compose$.composer.set('Typed then sent')
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

    // The autosave is armed from typing when the user clicks send, and the
    // identity allocation outlasts the debounce.
    scheduleQuickReplyDraftSave()
    const send = sendReply()
    await allocating
    await new Promise((resolve) => setTimeout(resolve, 1400))

    // A draft landing here would have nothing pointing at it once the send
    // resets the box — the stranded copy this whole thing is about.
    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(false)

    releaseIdentity()
    await send
  })

  it('takes the draft out of the box the moment the send claims it', async () => {
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
    compose$.quickReplyDraftThreadId.set('t-1')
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

    // The send owns that draft and will discard it; the box must not still be
    // pointing at it while the user carries on typing.
    expect(compose$.quickReplyDraftId.get()).toBe('')
    expect(compose$.quickReplyDraftSaved.get()).toBe(false)
    expect(compose$.quickReplyDraftThreadId.get()).toBe('')

    releaseIdentity()
    await send
    const discarded = calls
      .filter((call) => call.command === 'mail.discardDraft')
      .map((call) => (call.payload as { draft_id?: string }).draft_id)
    expect(discarded).toEqual(['claimed-draft@example.com'])
  })

  it('does not hand the claimed draft to the full editor mid-send', async () => {
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
    compose$.composer.set('Something else entirely')
    openReplyInFullEditor()

    // Handing the tab the claimed id would have the send delete the draft out
    // from under the full editor.
    const tab = compose$.tabs.get().find((item) => item.kind === 'compose')
    expect(tab?.compose?.draftMessageId).not.toBe('claimed-draft@example.com')

    releaseIdentity()
    await send
  })

  it('re-arms the autosave when the send dies before it gets a guard', async () => {
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
    compose$.composer.set('Never saved anywhere')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'recovered-draft@example.com' }
        identityStarted()
        await new Promise<void>((_resolve, reject) => (failIdentity = () => reject(new Error('offline'))))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The click cancelled the debounce; nothing else re-arms it if the user
    // doesn't type again, and their reply is not saved anywhere.
    scheduleQuickReplyDraftSave()
    const send = sendReply().catch(() => undefined)
    await allocating
    failIdentity()
    await send
    await new Promise((resolve) => setTimeout(resolve, 1400))

    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(true)
  })
})
