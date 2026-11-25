import { ownSentCrypto } from '../lib/crypto'
import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { openDraftCompose, openReplyInFullEditor, replyToMessage, composedContent, textToHtml } from './compose'
import { formatFullTimestamp } from '../components/chat/messageHelpers'
import { compose$ } from './composeState'
import { cancelQuickReplyDraftSave } from './quickReply'
import { conversationHtmlBody, sendReply } from './quickReplySend'
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

describe('quoting the original in replies', () => {
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

  const original = (overrides: Partial<Message> = {}) =>
    message({
      id: 'root',
      account_id: 'acc-1',
      thread_id: 't-1',
      folder_id: 'INBOX',
      from_name: 'Them',
      from_addr: 'them@example.com',
      message_id: 'root@example.com',
      subject: 'Lunch',
      body: 'Lunch on Friday?\n\nJane',
      ...overrides,
    })
  const attribution = (m: Message) => `On ${formatFullTimestamp(m.date)}, Them <them@example.com> wrote:`
  const htmlAttribution = (m: Message) => attribution(m).replace('<', '&lt;').replace('>', '&gt;')
  const draft = () => compose$.tabs.get()[0]?.compose

  beforeEach(() => {
    cancelQuickReplyDraftSave()
    calls.length = 0
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.composer.set('')
    compose$.composerAttachments.set([])
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyFrom.set('')
    compose$.quickReplySignatureOff.set(false)
    mail$.messages.set([])
    mail$.threads.set([])
    mail$.folders.set([])
    mail$.foldersByAccount.set({})
    ui$.selectedThread.set('')
    ui$.selectedAccount.set('acc-1')
    accounts$.set([sendable])
    settings$.signature.set('')
    settings$.quoteReplies.set(true)
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.allocateIdentity') return { message_id: 'sent@example.com' }
            if (command === 'mail.folderList') return { folders: [] }
            if (command === 'mail.threadList') return { threads: [] }
            return {}
          },
        },
      },
    }
  })

  afterEach(() => {
    settings$.quoteReplies.set(false)
  })

  it('leaves replies unquoted by default', () => {
    settings$.quoteReplies.set(false)
    accounts$.set([{ ...sendable, conversation_html: false }])

    replyToMessage(original())

    expect(draft()?.text).toBe('')
  })

  it('folds a plain original away under the signature', () => {
    accounts$.set([{ ...sendable, conversation_html: false }])
    settings$.signature.set('<p>Ping</p>')
    const m = original()

    replyToMessage(m)

    expect(draft()?.rich).toBe(false)
    expect(draft()?.text).toBe('\n\nPing')
    expect(draft()?.quote?.text).toBe(`${attribution(m)}\n> Lunch on Friday?\n>\n> Jane`)
    expect(composedContent(draft()!)).toBe(`\n\nPing\n\n${attribution(m)}\n> Lunch on Friday?\n>\n> Jane`)
  })

  it('folds an HTML original away in a cite blockquote', () => {
    accounts$.set([{ ...sendable, conversation_html: false }])
    settings$.signature.set('<p>Ping</p>')
    const m = original({ body_html: '<p>Lunch on <b>Friday</b>?</p>' })

    replyToMessage(m)

    expect(draft()?.rich).toBe(true)
    expect(draft()?.html).toBe('<p></p><p>Ping</p>')
    expect(composedContent(draft()!)).toBe(
      `<p></p><p>Ping</p><p>${htmlAttribution(m)}</p><blockquote type="cite"><p>Lunch on <b>Friday</b>?</p></blockquote>`,
    )
  })

  it('folds the quote under the text carried from the quick reply into the full editor', () => {
    const m = original()
    mail$.threads.set([m])
    mail$.messages.set([m])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Sounds good')

    openReplyInFullEditor()

    expect(draft()?.text).toBe('Sounds good')
    expect(composedContent(draft()!)).toBe(`Sounds good\n\n${attribution(m)}\n> Lunch on Friday?\n>\n> Jane`)
  })

  it('marks a quote withheld from decrypted mail its sender is not shown to have written', () => {
    const unsigned = { ...ownSentCrypto(), own: false, signed: false, trust: 'unconfirmed' as const }
    const m = original({ crypto: unsigned })

    replyToMessage(m)
    expect(draft()?.quote).toBeUndefined()
    expect(draft()?.quoteWithheld).toBe(true)

    // The quick reply opened in the full editor says so too.
    compose$.tabs.set([])
    mail$.threads.set([m])
    mail$.messages.set([m])
    ui$.selectedThread.set('t-1')
    openReplyInFullEditor()
    expect(draft()?.quoteWithheld).toBe(true)

    // Signed by the sender's key: quoted, nothing to explain.
    compose$.tabs.set([])
    replyToMessage(original({ crypto: { ...unsigned, signed: true, trust: 'known' } }))
    expect(draft()?.quote).toBeDefined()
    expect(draft()?.quoteWithheld).toBeFalsy()

    // Nothing to quote (a message that failed to decrypt has no body): no note,
    // whose reason would be wrong.
    compose$.tabs.set([])
    replyToMessage(original({ crypto: { ...unsigned, problem: 'integrity' }, body: '', body_html: undefined }))
    expect(draft()?.quoteWithheld).toBeFalsy()

    // With quoting off nothing is withheld, so there is no note either.
    compose$.tabs.set([])
    settings$.quoteReplies.set(false)
    replyToMessage(m)
    expect(draft()?.quoteWithheld).toBeFalsy()
  })

  it('does not quote a saved quoted reply carried back into a reply twice', async () => {
    const m = original()
    mail$.threads.set([m])
    mail$.messages.set([m])
    ui$.selectedThread.set('t-1')
    const saved = `Sounds good\n\n${attribution(m)}\n> Lunch on`
    compose$.composer.set(saved)

    openReplyInFullEditor()
    expect(draft()?.text).toBe(saved)
    expect(draft()?.quote).toBeUndefined()

    compose$.tabs.set([])
    compose$.composer.set(saved)
    await sendReply()
    const sent = calls.find((call) => call.command === 'mail.send')?.payload as { body: string }
    expect(sent.body).toBe(saved)
  })

  // #154 test 1 on the Linux build: a new rich message keeps its signature on
  // the server as `<p>typed</p><p>signature</p>` (typing fills the separator
  // paragraph), and the core hands the body back as a whole document.
  it('reopens a rich draft that ends in the signature with the switch on', async () => {
    settings$.signature.set('<p>-- <br><em>Studio</em><br>Signal: x.01</p>')
    const saved = message({
      id: 'acc-1#Drafts#7#1',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 'acc-1#Drafts#7',
      message_id: 'draft-sig@example.com',
      body: 'Testing this.\n\n--\nStudio\nSignal: x.01',
      body_html:
        '<!doctype html><html><head><meta charset="utf-8"><style>img{max-width:100%}</style></head><body><p>Testing this. </p><p>-- <br><em>Studio</em><br>Signal: x.01</p></body></html>',
    })
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') return { messages: [saved] }
      return {}
    }

    await openDraftCompose(saved)

    expect(draft()?.html).toBe('<p>Testing this. </p><p>-- <br><em>Studio</em><br>Signal: x.01</p>')
    expect(draft()?.signature).toEqual({
      html: '<p>-- <br><em>Studio</em><br>Signal: x.01</p>',
      text: '--\nStudio\nSignal: x.01',
      placement: 'belowText',
    })
  })

  it('folds the quote of a reopened draft away again', async () => {
    const plain = message({
      id: 'acc-1#Drafts#1#1',
      account_id: 'acc-1',
      folder_id: 'Drafts',
      thread_id: 'acc-1#Drafts#1',
      message_id: 'draft-plain@example.com',
      body: 'Sounds good\n\nOn Mon, Them wrote:\n> Lunch?',
      body_quote_start: 13,
    })
    const rich = message({
      ...plain,
      id: 'acc-1#Drafts#2#1',
      thread_id: 'acc-1#Drafts#2',
      message_id: 'draft-rich@example.com',
      body_html:
        '<p>Sounds good</p><p data-meron-quote="">On Mon, Them wrote:</p><blockquote type="cite" data-meron-quote=""><p>Lunch?</p></blockquote>',
    })
    ;(window as any).go.main.App.Invoke = async (command: string, payload: any) => {
      calls.push({ command, payload })
      if (command === 'mail.threadRead') return { messages: [payload.thread_id === plain.thread_id ? plain : rich] }
      return {}
    }

    await openDraftCompose(plain)
    expect(draft()?.text).toBe('Sounds good')
    expect(draft()?.quote?.text).toBe('On Mon, Them wrote:\n> Lunch?')

    compose$.tabs.set([])
    await openDraftCompose(rich)
    expect(draft()?.html).toBe('<p>Sounds good</p>')
    expect(draft()?.quote?.html).toBe('<p>On Mon, Them wrote:</p><blockquote type="cite"><p>Lunch?</p></blockquote>')

    // A quote that shares a wrapper with the reply.
    compose$.tabs.set([])
    rich.body_html = `<div>${rich.body_html}</div>`
    rich.message_id = 'draft-wrapped@example.com'
    await openDraftCompose(rich)
    expect(draft()?.html).toBe('<div><p>Sounds good</p></div>')
    expect(draft()?.quote?.html).toBe(
      '<div><p>On Mon, Them wrote:</p><blockquote type="cite"><p>Lunch?</p></blockquote></div>',
    )

    // An Outlook quote whose header sits in a wrapper and whose body runs past it.
    compose$.tabs.set([])
    rich.body_html =
      '<div><p>Sounds good</p><div id="divRplyFwdMsg" data-meron-quote="">From: Them</div></div><p data-meron-quote="">Lunch?</p>'
    rich.message_id = 'draft-outlook@example.com'
    await openDraftCompose(rich)
    expect(draft()?.html).toBe('<div><p>Sounds good</p></div>')
    expect(draft()?.quote?.html).toBe('<div><div id="divRplyFwdMsg">From: Them</div></div><p>Lunch?</p>')
  })

  it('sends the quote with a quick reply but keeps it out of the bubble', async () => {
    const m = original()
    mail$.threads.set([m])
    mail$.messages.set([m])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Sounds good')

    await sendReply()

    const sent = calls.find((call) => call.command === 'mail.send')?.payload as { body: string; html: string }
    expect(sent.body).toBe(`Sounds good\n\n${attribution(m)}\n> Lunch on Friday?\n>\n> Jane`)
    expect(sent.html).toBe('')
    expect(mail$.messages.get().find((message) => message.send_status)?.body).toBe('Sounds good')
  })

  it('sends an HTML quote with a quick reply to an HTML original', async () => {
    const m = original({ body_html: '<p>Lunch on <b>Friday</b>?</p>' })
    mail$.threads.set([m])
    mail$.messages.set([m])
    ui$.selectedThread.set('t-1')
    compose$.composer.set('Sounds good')

    await sendReply()

    const sent = calls.find((call) => call.command === 'mail.send')?.payload as { body: string; html: string }
    expect(sent.body).toBe(`Sounds good\n\n${attribution(m)}\n> Lunch on Friday?\n>\n> Jane`)
    expect(sent.html).toStartWith(`<p>Sounds good</p><p>${htmlAttribution(m)}</p><blockquote `)
    expect(sent.html).toContain('type="cite"')
    expect(sent.html).toContain('<p>Lunch on <b>Friday</b>?</p></blockquote>')
  })
})

describe('textToHtml', () => {
  it('links bare URLs, leaving trailing punctuation outside', () => {
    expect(textToHtml('same as https://github.com/nonbili/meron/releases/tag/android%2Fv0.5.0')).toBe(
      '<p>same as <a href="https://github.com/nonbili/meron/releases/tag/android%2Fv0.5.0">https://github.com/nonbili/meron/releases/tag/android%2Fv0.5.0</a></p>',
    )
    expect(textToHtml('(see www.example.com/a?x=1&y=2.)')).toBe(
      '<p>(see <a href="https://www.example.com/a?x=1&amp;y=2">www.example.com/a?x=1&amp;y=2</a>.)</p>',
    )
    expect(textToHtml('<https://x.example/(a)>')).toBe(
      '<p>&lt;<a href="https://x.example/(a)">https://x.example/(a)</a>&gt;</p>',
    )
  })

  it('leaves non-URLs as escaped text', () => {
    expect(textToHtml('https:// and www.nodot and me@www.example.com <b>')).toBe(
      '<p>https:// and www.nodot and me@www.example.com &lt;b&gt;</p>',
    )
  })
})

describe('quick reply inline images', () => {
  // A 37x21 PNG, and a 898x2014 phone screenshot header (only IHDR is read).
  const small =
    'iVBORw0KGgoAAAANSUhEUgAAACUAAAAVAQMAAAD7KrXtAAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAADUExURf8AABniCTcAAAAHdElNRQfqCw4MOS9AS53hAAAAJXRFWHRkYXRlOmNyZWF0ZQAyMDI2LTExLTE0VDEyOjU3OjQ3KzAwOjAwsErT2gAAACV0RVh0ZGF0ZTptb2RpZnkAMjAyNi0xMS0xNFQxMjo1Nzo0NyswMDowMMEXa2YAAAAodEVYdGRhdGU6dGltZXN0YW1wADIwMjYtMTEtMTRUMTI6NTc6NDcrMDA6MDCWAkq5AAAADElEQVQI12NgGEgAAAB+AAH9tTdSAAAAAElFTkSuQmCC'
  const tallBytes = new Uint8Array([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52, 0, 0, 0x03, 0x82, 0, 0, 0x07,
    0xde,
  ])
  const tall = btoa(String.fromCharCode(...tallBytes))
  const image = (inlineId: string, data: string) => ({
    id: inlineId,
    filename: `${inlineId}.png`,
    mime: 'image/png',
    size: data.length,
    data,
    inlineId,
  })

  it('puts every image in one paragraph, sized to fit without enlarging', () => {
    expect(conversationHtmlBody('hi', [image('a', tall), image('b', small)])).toBe(
      '<p>hi</p><p>' +
        '<img src="cid:a" alt="a.png" width="214" height="480" style="max-width:100%;height:auto"> ' +
        '<img src="cid:b" alt="b.png" width="37" height="21" style="max-width:100%;height:auto">' +
        '</p>',
    )
  })

  it('leaves an image of unknown size unsized', () => {
    expect(conversationHtmlBody('hi', [image('c', btoa('????'))])).toBe(
      '<p>hi</p><p><img src="cid:c" alt="c.png" style="max-width:100%;height:auto"></p>',
    )
  })
})
