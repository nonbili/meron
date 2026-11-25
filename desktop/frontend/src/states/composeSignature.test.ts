import { beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { openReplyInFullEditor, openComposeTab, updateComposeDraft } from './compose'
import { compose$ } from './composeState'
import {
  isQuickReplyBlank,
  quickReplyCaretOffset,
  saveQuickReplyDraft,
  startFreshQuickReply,
  quickReplySignature,
  quickReplyOutgoingText,
} from './quickReply'
import { sendReply } from './quickReplySend'
import { accounts$ } from './accounts'
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

describe('signatures in a new compose tab', () => {
  const sendable = {
    id: 'acc-1',
    email: 'me@example.com',
    display_name: 'Me',
    provider: 'custom',
    auth_type: 'password' as const,
    imap_host: 'imap.example.com',
    imap_port: 993,
    smtp_host: 'smtp.example.com',
    smtp_port: 465,
    tls: true,
  }

  beforeEach(() => {
    compose$.tabs.set([])
    compose$.activeTab.set('')
    ui$.selectedAccount.set('acc-1')
    accounts$.set([sendable])
    settings$.signature.set('')
  })

  const draftOf = (id: string | undefined) => compose$.tabs.get().find((tab) => tab.id === id)?.compose

  it('seeds a rich draft with the app-wide signature', () => {
    settings$.signature.set('<p>Ping</p>')

    expect(draftOf(openComposeTab())?.html).toBe('<p></p><p>Ping</p>')
  })

  it('lets an account override or opt out', () => {
    settings$.signature.set('<p>Ping</p>')
    accounts$.set([{ ...sendable, signature: { mode: 'custom', html: '<p>Mine</p>' } }])
    expect(draftOf(openComposeTab())?.html).toBe('<p></p><p>Mine</p>')

    accounts$.set([{ ...sendable, signature: { mode: 'none', html: '<p>Mine</p>' } }])
    expect(draftOf(openComposeTab())?.html).toBe('')
  })

  it('skips the signature when re-opening an existing message', () => {
    settings$.signature.set('<p>Ping</p>')

    expect(draftOf(openComposeTab({ noSignature: true }))?.html).toBe('')
  })
})

describe('signature on a change of From account', () => {
  const account = (id: string, signature?: { mode: 'global' | 'none' | 'custom'; html: string }) => ({
    id,
    email: `${id}@example.com`,
    display_name: id,
    provider: 'custom',
    auth_type: 'password' as const,
    imap_host: 'imap.example.com',
    imap_port: 993,
    smtp_host: 'smtp.example.com',
    smtp_port: 465,
    tls: true,
    signature,
  })

  beforeEach(() => {
    compose$.tabs.set([])
    compose$.activeTab.set('')
    ui$.selectedAccount.set('a')
    settings$.signature.set('')
    accounts$.set([
      account('a', { mode: 'custom', html: '<p>A</p>' }),
      account('b', { mode: 'custom', html: '<p>B</p>' }),
      account('c', { mode: 'none', html: '' }),
    ])
  })

  const draftOf = (id: string | undefined) => compose$.tabs.get().find((tab) => tab.id === id)?.compose

  it("replaces the old account's signature with the new one's", () => {
    const id = openComposeTab()!
    expect(draftOf(id)?.html).toBe('<p></p><p>A</p>')

    updateComposeDraft(id, { accountId: 'b', fromEmail: 'b@example.com' })

    expect(draftOf(id)?.html).toBe('<p></p><p>B</p>')
    expect(draftOf(id)?.signature?.html).toBe('<p>B</p>')
  })

  it('drops it when the new account sends none', () => {
    const id = openComposeTab()!
    updateComposeDraft(id, { accountId: 'c' })

    // The blank line the signature came with goes too, so switching accounts
    // repeatedly cannot pile up empty paragraphs.
    expect(draftOf(id)?.html).toBe('')
    // Not "unmanaged": the app knows this body has no signature and where one
    // would go, so moving to an account that has one puts it back there.
    expect(draftOf(id)?.signature).toEqual({ html: '', text: '', placement: 'belowText' })

    updateComposeDraft(id, { accountId: 'b' })
    expect(draftOf(id)?.html).toBe('<p></p><p>B</p>')
  })

  it('follows the draft through a switch to plaintext', () => {
    const id = openComposeTab()!
    // What the composer's rich/plain toggle does: the body becomes text, and
    // the tracked signature has to be findable in that form too.
    updateComposeDraft(id, { rich: false, html: '', text: '\n\nA' })
    updateComposeDraft(id, { accountId: 'b' })

    expect(draftOf(id)?.text).toBe('\n\nB')
  })

  it('leaves a reopened draft body alone, rather than giving it two signatures', () => {
    // "Edit as New Message" and saved drafts come with a body this app did not
    // compose; it may already hold a signature, here with a line added below.
    const id = openComposeTab({ rich: true, html: '<p>hello</p><p>A</p><p>PS</p>', noSignature: true })!
    expect(draftOf(id)?.signature).toBeUndefined()

    updateComposeDraft(id, { accountId: 'b' })

    expect(draftOf(id)?.html).toBe('<p>hello</p><p>A</p><p>PS</p>')
    expect(draftOf(id)?.signature).toBeUndefined()
  })

  // A saved new message keeps no empty paragraph above its signature: the first
  // words were typed into it. Ending in the account's signature, it is tracked,
  // so a From change swaps it for the other account's rather than adding one.
  it("swaps the signature a reopened draft ends in, as a composed one's", () => {
    const id = openComposeTab({ rich: true, html: '<p>hello</p><p>A</p>', noSignature: true })!
    expect(draftOf(id)?.signature).toEqual({ html: '<p>A</p>', text: 'A', placement: 'belowText' })

    updateComposeDraft(id, { accountId: 'b' })

    expect(draftOf(id)?.html).toBe('<p>hello</p><p>B</p>')
  })

  it('leaves the body alone once the signature has been edited', () => {
    const id = openComposeTab()!
    updateComposeDraft(id, { html: '<p></p><p>A, but mine now</p>' })
    updateComposeDraft(id, { accountId: 'b' })

    expect(draftOf(id)?.html).toBe('<p></p><p>A, but mine now</p>')
  })

  it('does not touch the body when the account is unchanged', () => {
    const id = openComposeTab()!
    updateComposeDraft(id, { accountId: 'a', fromEmail: 'alias@example.com' })

    expect(draftOf(id)?.html).toBe('<p></p><p>A</p>')
  })
})

describe('signature in the quick reply', () => {
  const calls: { command: string; payload: unknown }[] = []

  const sendable = {
    id: 'acc-1',
    email: 'me@example.com',
    display_name: 'Me',
    provider: 'custom',
    auth_type: 'password' as const,
    imap_host: 'imap.example.com',
    imap_port: 993,
    smtp_host: 'smtp.example.com',
    smtp_port: 465,
    tls: true,
  }

  const setUpThread = () => {
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
    return thread
  }

  beforeEach(() => {
    calls.length = 0
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.composer.set('')
    compose$.composerAttachments.set([])
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyFrom.set('')
    compose$.quickReplySignatureOff.set(false)
    compose$.quickReplySignatureUnmanaged.set(false)
    mail$.messages.set([])
    mail$.threads.set([])
    mail$.folders.set([])
    mail$.foldersByAccount.set({})
    ui$.selectedThread.set('')
    ui$.selectedAccount.set('acc-1')
    accounts$.set([sendable])
    settings$.signature.set('<p>from u2</p>')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.allocateIdentity') return { message_id: 'draft-core@example.com' }
            return {}
          },
        },
      },
    }
  })

  const savedBody = () =>
    (calls.filter((c) => c.command === 'mail.saveDraft').at(-1)?.payload as { body?: string } | undefined)?.body

  const hydrate = (body: string) => {
    const thread = message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
    })
    mail$.folders.set([{ id: 'Drafts', account_id: 'acc-1', name: 'Drafts', kind: 'drafts' } as any])
    mail$.foldersByAccount.set({ 'acc-1': [{ id: 'Drafts', name: 'Drafts', kind: 'drafts' } as any] })
    mail$.threads.set([thread])
    ui$.selectedThread.set('t-1')
    mail$.messages.set([
      thread,
      message({
        id: 'd1',
        account_id: 'acc-1',
        thread_id: 't-1',
        folder_id: 'Drafts',
        to: 'them@example.com',
        subject: 'Re: Notification thread',
        outgoing: true,
        message_id: 'draft@example.com',
        body,
        date: thread.date + 60,
      }),
    ])
  }

  it('starts with an empty box, with the signature on', () => {
    setUpThread()
    compose$.composer.set('left over')
    compose$.quickReplySignatureOff.set(true)
    startFreshQuickReply()

    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplySignatureOff.get()).toBe(false)
    expect(quickReplySignature().text).toBe('from u2')
  })

  it('counts an untouched box as blank, and does not save a draft for it', async () => {
    setUpThread()
    startFreshQuickReply()

    expect(isQuickReplyBlank()).toBe(true)
    await saveQuickReplyDraft()
    expect(calls.filter((c) => c.command === 'mail.saveDraft')).toHaveLength(0)
  })

  it('saves and sends the reply with the signature under it', async () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('t2')

    expect(quickReplyOutgoingText()).toBe('t2\n\nfrom u2')
    await saveQuickReplyDraft()
    expect(savedBody()).toBe('t2\n\nfrom u2')
  })

  it("sends the user's own whitespace exactly as written", async () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('    indented start\n\nand a trailing blank line\n')

    await saveQuickReplyDraft()
    expect(savedBody()).toBe('    indented start\n\nand a trailing blank line\n\n\nfrom u2')
  })

  it('sends the signature alone with an attachment-only reply', async () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composerAttachments.set([{ id: 'a1', filename: 'x.png', mime: 'image/png', size: 1, data: 'AA==' }])

    await saveQuickReplyDraft()
    expect(savedBody()).toBe('from u2')
  })

  it('leaves the signature off when the user switched it off', async () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('t2')
    compose$.quickReplySignatureOff.set(true)

    await saveQuickReplyDraft()
    expect(savedBody()).toBe('t2')
  })

  it('adds nothing for an account that sends no signature', async () => {
    accounts$.set([{ ...sendable, signature: { mode: 'none', html: '' } }])
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('t2')

    expect(quickReplySignature().text).toBe('')
    await saveQuickReplyDraft()
    expect(savedBody()).toBe('t2')
  })

  it('signs with the signature as it is now, not as it was when the box opened', async () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('t2')
    settings$.signature.set('<p>changed</p>')

    await saveQuickReplyDraft()
    expect(savedBody()).toBe('t2\n\nchanged')
  })

  it('puts the caret at the end of the text', () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('t2')
    expect(quickReplyCaretOffset()).toBe(2)
  })

  // A saved draft holds the reply as it goes out, signature included. Loading it
  // back takes off only a signature this app can recognise, so sending it again
  // never adds a second one (#149).
  it('takes the signature off a hydrated draft that ends in it, and puts it back on send', async () => {
    hydrate('half typed\n\nfrom u2')

    expect(compose$.composer.get()).toBe('half typed')
    expect(compose$.quickReplySignatureOff.get()).toBe(false)
    expect(quickReplyOutgoingText()).toBe('half typed\n\nfrom u2')
  })

  it('leaves a hydrated draft without the signature alone, and adds none on send', () => {
    hydrate('sent with it off')

    expect(compose$.composer.get()).toBe('sent with it off')
    expect(compose$.quickReplySignatureOff.get()).toBe(true)
    // No trace of the signature: off, and switchable back on.
    expect(compose$.quickReplySignatureUnmanaged.get()).toBe(false)
    expect(quickReplyOutgoingText()).toBe('sent with it off')
  })

  it('leaves a hydrated draft alone when text follows the signature', () => {
    hydrate('reply\n\nfrom u2\n\n> quoted')

    expect(compose$.composer.get()).toBe('reply\n\nfrom u2\n\n> quoted')
    expect(compose$.quickReplySignatureOff.get()).toBe(true)
    // Its signature is in the box: switching the current one on would add a second.
    expect(compose$.quickReplySignatureUnmanaged.get()).toBe(true)
    expect(quickReplyOutgoingText()).toBe('reply\n\nfrom u2\n\n> quoted')
  })

  it('leaves a hydrated draft alone when its signature has since changed', () => {
    settings$.signature.set('<p>new one</p>')
    hydrate('half typed\n\nfrom u2')

    expect(compose$.composer.get()).toBe('half typed\n\nfrom u2')
    expect(compose$.quickReplySignatureOff.get()).toBe(true)
    expect(quickReplyOutgoingText()).toBe('half typed\n\nfrom u2')
  })

  it('leaves a hydrated draft with an edited signature unmanaged, so none is added under it', () => {
    settings$.signature.set('<p>from u2</p><p>Phone 1</p>')
    hydrate('half typed\n\nfrom u2\nPhone 2')

    expect(compose$.composer.get()).toBe('half typed\n\nfrom u2\nPhone 2')
    expect(compose$.quickReplySignatureOff.get()).toBe(true)
    expect(compose$.quickReplySignatureUnmanaged.get()).toBe(true)
    expect(quickReplyOutgoingText()).toBe('half typed\n\nfrom u2\nPhone 2')
  })

  it('gives the full editor an unmanaged signature when the box holds an edited one', () => {
    settings$.signature.set('<p>from u2</p><p>Phone 1</p>')
    hydrate('half typed\n\nfrom u2\nPhone 2')

    openReplyInFullEditor()

    const draft = compose$.tabs.get()[0].compose
    expect(draft?.text).toBe('half typed\n\nfrom u2\nPhone 2')
    expect(draft?.signature).toBeUndefined()
    expect(compose$.quickReplySignatureUnmanaged.get()).toBe(false)
  })

  it('recognises the signature of a draft hydrated before the signature loaded', () => {
    settings$.signature.set('')
    hydrate('half typed\n\nfrom u2')
    expect(compose$.composer.get()).toBe('half typed\n\nfrom u2')

    settings$.signature.set('<p>from u2</p>')
    expect(compose$.composer.get()).toBe('half typed')
    expect(compose$.quickReplySignatureOff.get()).toBe(false)
    expect(quickReplyOutgoingText()).toBe('half typed\n\nfrom u2')
  })

  it('locks the switch of a draft hydrated before the signature loaded, once it shows an edited copy', () => {
    settings$.signature.set('')
    hydrate('half typed\n\nfrom u2\nPhone 2')
    expect(compose$.quickReplySignatureUnmanaged.get()).toBe(false)

    settings$.signature.set('<p>from u2</p><p>Phone 1</p>')
    expect(compose$.composer.get()).toBe('half typed\n\nfrom u2\nPhone 2')
    expect(compose$.quickReplySignatureOff.get()).toBe(true)
    expect(compose$.quickReplySignatureUnmanaged.get()).toBe(true)
  })

  it('does not touch a hydrated draft the user has typed into when the signature arrives', () => {
    settings$.signature.set('')
    hydrate('half typed\n\nfrom u2')
    compose$.composer.set('half typed\n\nfrom u2 and more')

    settings$.signature.set('<p>from u2</p>')
    expect(compose$.composer.get()).toBe('half typed\n\nfrom u2 and more')
    expect(compose$.quickReplySignatureOff.get()).toBe(true)
  })

  it('gives the full editor the text, which inserts and tracks its own signature', () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('t2')

    openReplyInFullEditor()

    const draft = compose$.tabs.get()[0].compose
    expect(draft?.text).toBe('t2\n\nfrom u2')
    expect(draft?.signature).toEqual({ html: '<p>from u2</p>', text: 'from u2', placement: 'belowText' })
    // And the box behind the new tab is a fresh quick reply.
    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplySignatureOff.get()).toBe(false)
  })

  it('gives the full editor no signature when it was switched off', () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('t2')
    compose$.quickReplySignatureOff.set(true)

    openReplyInFullEditor()

    const draft = compose$.tabs.get()[0].compose
    expect(draft?.text).toBe('t2')
    // Off, but switchable back on in the full composer.
    expect(draft?.signature).toEqual({ html: '', text: '', placement: 'belowText', optedOut: true })
  })

  it('tracks the signature a reopened draft ends in, so it can be switched', () => {
    setUpThread()
    openComposeTab({ accountId: 'acc-1', text: 'Saved\n\nfrom u2', noSignature: true })

    const draft = compose$.tabs.get()[0].compose
    expect(draft?.text).toBe('Saved\n\nfrom u2')
    expect(draft?.signature).toEqual({ html: '<p>from u2</p>', text: 'from u2', placement: 'belowText' })
  })

  it('leaves a reopened draft unmanaged when its signature is there but not as inserted', () => {
    setUpThread()
    openComposeTab({ accountId: 'acc-1', text: 'Saved\n\nfrom u2\n\nPS', noSignature: true })
    expect(compose$.tabs.get()[0].compose?.signature).toBeUndefined()
    expect(compose$.tabs.get()[0].compose?.text).toBe('Saved\n\nfrom u2\n\nPS')
  })

  // #154 review, point 1: a draft saved with the switch off holds no signature,
  // and reopened it must stay switchable, as the quick reply's is.
  it('reopens a draft with no trace of the signature with the switch off, not disabled', () => {
    setUpThread()
    openComposeTab({ accountId: 'acc-1', text: 'Saved without it', noSignature: true })
    expect(compose$.tabs.get()[0].compose?.signature).toEqual({
      html: '',
      text: '',
      placement: 'belowText',
      optedOut: true,
    })
    expect(compose$.tabs.get()[0].compose?.text).toBe('Saved without it')
  })

  it('puts the switch back with the reply when the send dies before going out', async () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('Never left')
    compose$.quickReplySignatureOff.set(true)
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') throw new Error('offline')
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await expect(sendReply()).rejects.toThrow('offline')

    expect(compose$.composer.get()).toBe('Never left')
    expect(compose$.quickReplySignatureOff.get()).toBe(true)
  })

  it('puts an unmanaged switch back with the reply when the send dies before going out', async () => {
    settings$.signature.set('<p>from u2</p><p>Phone 1</p>')
    hydrate('Never left\n\nfrom u2\nPhone 2')
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') throw new Error('offline')
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await expect(sendReply()).rejects.toThrow('offline')

    expect(compose$.composer.get()).toBe('Never left\n\nfrom u2\nPhone 2')
    expect(compose$.quickReplySignatureUnmanaged.get()).toBe(true)
  })

  it('switches the signature back on for the next reply after a send', async () => {
    setUpThread()
    startFreshQuickReply()
    compose$.composer.set('Went out')
    compose$.quickReplySignatureOff.set(true)
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
      if (command === 'mail.folderList') return { folders: [] }
      if (command === 'mail.threadList') return { threads: [] }
      return {}
    }

    await sendReply()

    expect((calls.find((c) => c.command === 'mail.send')?.payload as { body?: string }).body).toBe('Went out')
    expect(compose$.composer.get()).toBe('')
    expect(compose$.quickReplySignatureOff.get()).toBe(false)
  })
})
