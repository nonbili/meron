import { useQuickReplyDraftFixture } from './quickReplyTestFixture'
import { describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { compose$ } from './composeState'
import { saveQuickReplyDraft } from './quickReply'
import { retrySend, sendReply } from './quickReplySend'
import { settings$ } from './settings'
import { ui$ } from './ui'
import { mail$ } from './mail'
import { deleteMessage } from './mailMoves'

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

  it('shows the sending bubble and empties the box before the identity is allocated', async () => {
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
    compose$.composer.set('Instant reply')
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
    // Synchronously at the click: the bubble is up and the box is empty.
    const bubble = mail$.messages.get().find((m) => m.send_status === 'sending')
    expect(bubble?.body).toBe('Instant reply')
    expect(bubble?.to).toBe('Sender <them@example.com>')
    expect(bubble?.message_id).toBe('')
    expect(compose$.composer.get()).toBe('')
    await allocating
    releaseIdentity()
    await send

    // Once allocated, the bubble carries the identity the send went out under.
    const settled = mail$.messages.get().find((m) => m.id === bubble?.id)
    expect(settled?.message_id).toBe('sent@example.com')
    expect(settled?.send_status).toBe('sent')
    const sent = calls.find((call) => call.command === 'mail.send')
    expect((sent?.payload as { message_id?: string })?.message_id).toBe('sent@example.com')
  })

  it('threads a sent-only conversation against the loaded message instead of its new bubble', async () => {
    const headerOnly = message({
      id: 'server-header',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'Sent',
      from_addr: 'me@example.com',
      to: 'them@example.com',
      message_id: '',
      outgoing: true,
      date: 1000,
      // Our own message: the core answers a follow-up to its recipients.
      reply: { to: 'them@example.com', cc: '', all_to: 'them@example.com', all_cc: '', all_adds_recipients: false },
    })
    const loaded = message({
      ...headerOnly,
      id: 'server-loaded',
      message_id: 'parent@example.com',
    })
    mail$.threads.set([headerOnly])
    mail$.messages.set([headerOnly])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Follow up')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') return { messages: [loaded] }
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await sendReply()

    expect(calls.filter((call) => call.command === 'mail.threadRead')).toHaveLength(1)
    const sent = calls.find((call) => call.command === 'mail.send')
    expect(sent?.payload).toMatchObject({
      to: 'them@example.com',
      in_reply_to: 'parent@example.com',
      references: 'parent@example.com',
    })
  })

  it('takes the bubble down and puts the reply back when the send dies before going out', async () => {
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
    compose$.composer.set('Never left')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') throw new Error('offline')
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await expect(sendReply()).rejects.toThrow('offline')

    expect(mail$.messages.get().some((m) => m.send_status)).toBe(false)
    expect(compose$.composer.get()).toBe('Never left')
    expect(calls.some((call) => call.command === 'mail.send')).toBe(false)
  })

  it('saves the reply as a draft when the send dies after the box moved on', async () => {
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
    compose$.composer.set('Never left')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'rescued-draft@example.com' }
        identityStarted()
        await new Promise<void>((resolve) => (failIdentity = resolve))
        throw new Error('offline')
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await allocating
    // The reply was never autosaved, and the user is already writing the next
    // one, so the box can't take it back.
    compose$.composer.set('Next reply')
    failIdentity()
    await expect(send).rejects.toThrow('offline')
    await new Promise((resolve) => setTimeout(resolve, 0))

    // It went to Drafts rather than disappearing with the bubble.
    const saved = calls.find((call) => call.command === 'mail.saveDraft')
    expect((saved?.payload as { body?: string })?.body).toBe('Never left')
    expect(mail$.messages.get().some((m) => m.send_status)).toBe(false)
    expect(compose$.composer.get()).toBe('Next reply')
    expect(calls.some((call) => call.command === 'mail.send')).toBe(false)
  })

  it('rescues a reply whose only draft is one a failed autosave may not have written', async () => {
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
    compose$.composer.set('Only copy')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let autosaveFails = true
    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'uncertain@example.com' }
        identityStarted()
        await new Promise<void>((resolve) => (failIdentity = resolve))
        throw new Error('offline')
      }
      if (command === 'mail.saveDraft' && autosaveFails) throw new Error('sidecar save_draft timeout')
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The autosave failed after allocating: the server may or may not hold it.
    await saveQuickReplyDraft()
    expect(compose$.quickReplyDraftId.get()).toBe('uncertain@example.com')
    autosaveFails = false
    const callsBeforeSend = calls.length

    const send = sendReply()
    await allocating
    compose$.composer.set('Next reply')
    failIdentity()
    await expect(send).rejects.toThrow('offline')
    await new Promise((resolve) => setTimeout(resolve, 0))

    // The uncertain copy was no safety net, so the reply was rescued — under the
    // same id, overwriting whatever the failed autosave may have left.
    const rescues = calls.slice(callsBeforeSend).filter((call) => call.command === 'mail.saveDraft')
    expect(rescues).toHaveLength(1)
    const rescue = rescues[0]
    expect((rescue?.payload as { body?: string })?.body).toBe('Only copy')
    expect((rescue?.payload as { draft_id?: string })?.draft_id).toBe('uncertain@example.com')
    expect(compose$.composer.get()).toBe('Next reply')
  })

  // The rescue keeps the encryption the reply was sent with, not that of the
  // thread the user opened meanwhile: a reply to encrypted mail is never
  // saved plain because a plain thread is now open.
  it('rescues an encrypted reply encrypted after the user opened a plain thread', async () => {
    const encrypted = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
      crypto: { encrypted: true },
    } as Partial<Message>)
    const plain = message({
      id: 'other',
      account_id: 'acc-1',
      thread_id: 't-2',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'other@example.com',
      date: 2000,
    })
    mail$.threads.set([encrypted, plain])
    mail$.messages.set([encrypted])
    ui$.selectedThread.set('t-1')
    compose$.quickReplyEncrypt.set(null)
    compose$.composer.set('Secret reply')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'rescued@example.com' }
        identityStarted()
        await new Promise<void>((resolve) => (failIdentity = resolve))
        throw new Error('offline')
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }
    const callsBeforeSend = calls.length

    const send = sendReply()
    await allocating
    ui$.selectedThread.set('t-2')
    mail$.messages.set([plain])
    failIdentity()
    await expect(send).rejects.toThrow('offline')
    await new Promise((resolve) => setTimeout(resolve, 0))

    const rescues = calls.slice(callsBeforeSend).filter((call) => call.command === 'mail.saveDraft')
    expect(rescues).toHaveLength(1)
    expect(rescues[0].payload).toMatchObject({ body: 'Secret reply', encrypt: true, plain: false })
  })

  it('retries the rescue from the failed bubble when the draft save fails too', async () => {
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
    compose$.composer.set('Never left')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let backendDown = true
    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) {
          if (backendDown) throw new Error('offline')
          return { message_id: 'rescued-draft@example.com' }
        }
        identityStarted()
        await new Promise<void>((resolve) => (failIdentity = resolve))
        throw new Error('offline')
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await allocating
    compose$.composer.set('Next reply')
    failIdentity()
    await expect(send).rejects.toThrow('offline')
    await new Promise((resolve) => setTimeout(resolve, 0))

    // The rescue couldn't reach the backend either, so the bubble stays as the
    // last copy of the reply — marked failed, with Retry live.
    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(false)
    const bubble = mail$.messages.get().find((m) => m.send_status === 'failed')
    expect(bubble?.body).toBe('Never left')

    backendDown = false
    await retrySend(bubble!.id)

    const saved = calls.find((call) => call.command === 'mail.saveDraft')
    expect((saved?.payload as { body?: string })?.body).toBe('Never left')
    expect(mail$.messages.get().some((m) => m.send_status)).toBe(false)
  })

  it('brings a failed rescue bubble back when its thread is reopened', async () => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      date: 1000,
    })
    const elsewhere = message({
      id: 'other-root',
      account_id: 'acc-1',
      thread_id: 't-2',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'other-root@example.com',
      date: 2000,
    })
    mail$.threads.set([thread, elsewhere])
    mail$.messages.set([thread])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Never left')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let backendDown = true
    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) {
          if (backendDown) throw new Error('offline')
          return { message_id: 'rescued-draft@example.com' }
        }
        identityStarted()
        await new Promise<void>((resolve) => (failIdentity = resolve))
        throw new Error('offline')
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await allocating
    compose$.composer.set('Next reply')
    failIdentity()
    await expect(send).rejects.toThrow('offline')
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(mail$.messages.get().some((m) => m.send_status === 'failed')).toBe(true)

    // Off to another thread and back: the message page is rebuilt from the
    // server, which knows nothing about this reply.
    ui$.selectedThread.set('t-2')
    mail$.messages.set([elsewhere])
    await new Promise((resolve) => setTimeout(resolve, 0))
    ui$.selectedThread.set('t-1')
    mail$.messages.set([thread])
    await new Promise((resolve) => setTimeout(resolve, 0))

    const restored = mail$.messages.get().find((m) => m.send_status === 'failed')
    expect(restored?.body).toBe('Never left')

    // And it is still the live rescue, not just a rendering of the text.
    backendDown = false
    await retrySend(restored!.id)
    const saved = calls.find((call) => call.command === 'mail.saveDraft')
    expect((saved?.payload as { body?: string })?.body).toBe('Never left')
    expect(mail$.messages.get().some((m) => m.send_status)).toBe(false)
  })

  it('undoes the rescue when the failed bubble is deleted while it is saving', async () => {
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
    compose$.composer.set('Never left')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    let saveStarted!: () => void
    const saving = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'rescued-draft@example.com' }
        identityStarted()
        await new Promise<void>((resolve) => (failIdentity = resolve))
        throw new Error('offline')
      }
      if (command === 'mail.saveDraft') {
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    const send = sendReply()
    await allocating
    compose$.composer.set('Next reply')
    failIdentity()
    await expect(send).rejects.toThrow('offline')

    // The rescue is on the wire when the user deletes the bubble it belongs to.
    await saving
    const bubble = mail$.messages.get().find((m) => m.send_status === 'failed')
    await deleteMessage(bubble!)
    releaseSave()
    await new Promise((resolve) => setTimeout(resolve, 0))

    // The copy it wrote goes with the bubble rather than turning up in Drafts.
    const discard = calls.find((call) => call.command === 'mail.discardDraft')
    expect((discard?.payload as { draft_id?: string })?.draft_id).toBe('rescued-draft@example.com')

    // And the deleted bubble stays deleted across a reload of the thread.
    mail$.messages.set([thread])
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(mail$.messages.get().some((m) => m.send_status)).toBe(false)
  })

  it('lets the autosave it overtook rescue the reply instead of saving a second draft', async () => {
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
    compose$.composer.set('Never left')
    compose$.quickReplySignatureOff.set(false)
    settings$.signature.set('')

    let saveGateUsed = false
    let saveStarted!: () => void
    const started = new Promise<void>((resolve) => (saveStarted = resolve))
    let releaseSave!: () => void
    let identityStarted!: () => void
    const allocating = new Promise<void>((resolve) => (identityStarted = resolve))
    let failIdentity!: () => void
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') {
        if ((payload as { draft?: boolean }).draft) return { message_id: 'raced-draft@example.com' }
        identityStarted()
        await new Promise<void>((resolve) => (failIdentity = resolve))
        throw new Error('offline')
      }
      // Only the first save is held open; a rescue save racing it must not
      // hang the test that is meant to prove it never happens.
      if (command === 'mail.saveDraft' && !saveGateUsed) {
        saveGateUsed = true
        saveStarted()
        await new Promise<void>((resolve) => (releaseSave = resolve))
      }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    // The autosave of this reply is on the wire — its id isn't back yet — when
    // the send takes the box and dies, with the user already typing the next.
    const save = saveQuickReplyDraft()
    await started
    const send = sendReply()
    await allocating
    compose$.composer.set('Next reply')
    failIdentity()
    await expect(send).rejects.toThrow('offline')
    releaseSave()
    await save
    await new Promise((resolve) => setTimeout(resolve, 0))

    // That save's copy is the reply: no rescue draft beside it, and nothing
    // discarded it.
    expect(calls.filter((call) => call.command === 'mail.saveDraft')).toHaveLength(1)
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)
    expect(mail$.messages.get().some((m) => m.send_status)).toBe(false)
    expect(compose$.composer.get()).toBe('Next reply')
  })
})
