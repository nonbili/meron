import { useQuickReplyDraftFixture } from './quickReplyTestFixture'
import { afterEach, describe, expect, it } from 'bun:test'
import type { Message } from '../types'
import { quoteSelectionInReply, selectionQuote } from './compose'
import { compose$ } from './composeState'
import { mail$ } from './mail'
import { openThreadTab } from './messageTabs'
import { messageSelection } from '../components/chat/messageHelpers'
import { settings$ } from './settings'
import { ui$ } from './ui'

const message = (overrides: Partial<Message> = {}): Message => ({
  id: 'm1',
  account_id: 'acc-1',
  folder_id: 'INBOX',
  thread_id: 't-1',
  from_name: 'Erika Muster',
  from_addr: 'erika@example.com',
  to: 'me@example.com',
  subject: 'Wishes',
  preview: '',
  body: 'Hallo,\n\n- Konversationansicht ausschaltbar.\n- Dunkles Design.\n',
  message_id: 'm1@example.com',
  date: 1000,
  unread: false,
  starred: false,
  has_attachments: false,
  ...overrides,
})

describe('selectionQuote', () => {
  it('quotes each line, dropping the blank lines around the passage', () => {
    expect(selectionQuote('\n  \nFirst line  \n\nSecond\r\n\n').text).toBe('> First line\n>\n> Second')
  })

  it('escapes the passage in its HTML form', () => {
    expect(selectionQuote('a <b> & c').html).toBe('<blockquote type="cite"><p>a &lt;b&gt; &amp; c</p></blockquote>')
  })

  it('is empty for a selection of whitespace', () => {
    expect(selectionQuote(' \n \n').text).toBe('')
  })
})

describe('quoteSelectionInReply', () => {
  useQuickReplyDraftFixture()
  afterEach(() => settings$.conversationFooter.set('reply'))

  const openConversation = (target: Message) => {
    mail$.threads.set([target])
    mail$.messages.set([target])
    ui$.selectedThread.set(target.thread_id)
    settings$.conversationFooter.set('reply')
  }

  it('adds the quoted passage to the quick reply, under what it already holds', () => {
    const target = message()
    openConversation(target)
    compose$.composer.set('Thanks!\n')
    const focus = ui$.replyFocus.peek()

    quoteSelectionInReply(target, '- Konversationansicht ausschaltbar.')
    quoteSelectionInReply(target, '- Dunkles Design.')

    expect(compose$.composer.get()).toBe('Thanks!\n\n> - Konversationansicht ausschaltbar.\n\n> - Dunkles Design.\n\n')
    expect(ui$.replyFocus.peek()).toBe(focus + 2)
    expect(compose$.tabs.get()).toHaveLength(0)
  })

  it('opens a reply in a compose tab where the conversation has no reply box', () => {
    const target = message()
    openConversation(target)
    settings$.conversationFooter.set('actions')

    quoteSelectionInReply(target, 'Dunkles Design.')

    const tabs = compose$.tabs.get()
    expect(tabs).toHaveLength(1)
    expect(tabs[0].compose?.text).toStartWith('> Dunkles Design.\n\n')
    expect(tabs[0].compose?.inReplyTo).toBe('m1@example.com')
    expect(compose$.composer.get()).toBe('')
  })

  it('quotes into the quick reply of a conversation open in a thread tab', () => {
    const target = message()
    openConversation(target)
    openThreadTab(target)

    quoteSelectionInReply(target, 'Dunkles Design.')

    expect(compose$.composer.get()).toBe('> Dunkles Design.\n\n')
    expect(compose$.tabs.get().filter((tab) => tab.kind === 'compose')).toHaveLength(0)
  })

  it('opens a compose tab for Local Folders mail, which shows no reply box', () => {
    const target = message({ account_id: 'local' })
    openConversation(target)

    quoteSelectionInReply(target, 'Dunkles Design.')

    expect(compose$.composer.get()).toBe('')
    expect(compose$.tabs.get().filter((tab) => tab.kind === 'compose')).toHaveLength(1)
  })

  it('keeps a passage of encrypted mail out of the quick reply, in a reply that stays encrypted', () => {
    const target = message({
      crypto: {
        encrypted: true,
        signed: true,
        trust: 'known',
        signer: 'C0FFEE',
        partial: false,
        weak_hash: false,
        key_changed: false,
        verified_conflict: false,
        on_list: false,
        own: false,
        signer_stored: true,
        offer: false,
      },
    })
    openConversation(target)

    quoteSelectionInReply(target, 'the secret')

    expect(compose$.composer.get()).toBe('')
    const tabs = compose$.tabs.get()
    expect(tabs).toHaveLength(1)
    expect(tabs[0].compose?.decryptedInBody).toBe(true)
  })
})

describe('messageSelection', () => {
  afterEach(() => {
    window.getSelection()?.removeAllRanges()
    document.body.innerHTML = ''
  })

  const select = (from: Node, to: Node) => {
    const range = document.createRange()
    range.setStart(from, 0)
    range.setEnd(to, to.textContent?.length ?? 0)
    window.getSelection()?.removeAllRanges()
    window.getSelection()?.addRange(range)
  }

  it('takes a selection lying within the message', () => {
    document.body.innerHTML = '<div id="a"><p>plain words</p></div><div id="b"><p>the secret</p></div>'
    const a = document.getElementById('a')!
    select(a.firstChild!.firstChild!, a.firstChild!.firstChild!)
    expect(messageSelection({ nativeEvent: new MouseEvent('contextmenu') }, a)).toBe('plain words')
  })

  it('ignores a selection dragged on into another message', () => {
    document.body.innerHTML = '<div id="a"><p>plain words</p></div><div id="b"><p>the secret</p></div>'
    const a = document.getElementById('a')!
    const b = document.getElementById('b')!
    select(a.firstChild!.firstChild!, b.firstChild!.firstChild!)
    expect(messageSelection({ nativeEvent: new MouseEvent('contextmenu') }, a)).toBe('')
    expect(messageSelection({ nativeEvent: new MouseEvent('contextmenu') }, b)).toBe('')
  })
})
