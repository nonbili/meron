import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { compose$ } from '../../states/composeState'
import { cancelQuickReplyDraftSave } from '../../states/quickReply'
import { mail$ } from '../../states/mail'
import { settings$ } from '../../states/settings'
import { ui$ } from '../../states/ui'
import { kanban$ } from '../../states/kanban'
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

// A stand-in for QuickReplyComposer: just the hook and its textarea, so the
// key handler runs against a real element.
function ReplyBox() {
  const { textareaRef, handleComposerKeyDown } = useQuickReply()
  return <textarea data-testid="reply" ref={textareaRef} onKeyDown={handleComposerKeyDown} />
}

describe('useQuickReply Escape in the kanban pane', () => {
  const calls: Array<{ command: string; payload: any }> = []
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
    compose$.tabs.set([])
    compose$.activeTab.set('')
    compose$.composer.set('')
    compose$.composerAttachments.set([])
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyDraftThreadId.set('')
    compose$.quickReplyFrom.set('')
    compose$.quickReplySignature.set(null)
    kanban$.activeBoardId.set('board-1')
    kanban$.paneThreadId.set('t-1')
    kanban$.paneColumnKey.set('board-1:acc-1:INBOX')
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
    kanban$.activeBoardId.set('')
    kanban$.paneThreadId.set('')
    kanban$.paneColumnKey.set('')
    compose$.tabs.set([])
    compose$.activeTab.set('')
    ui$.selectedThread.set('')
    if (previousGo === undefined) delete (window as any).go
    else (window as any).go = previousGo
  })

  it('saves what was just typed at once and closes the pane', async () => {
    const { getByTestId } = render(<ReplyBox />)
    await act(async () => {
      await Promise.resolve()
      compose$.composer.set('Typed a moment ago')
    })

    await act(async () => {
      fireEvent.keyDown(getByTestId('reply'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 0))
    })

    expect(kanban$.paneThreadId.get()).toBe('')
    expect(ui$.selectedThread.get()).toBe('')
    const save = calls.find((call) => call.command === 'mail.saveDraft')
    expect(JSON.stringify(save?.payload)).toContain('Typed a moment ago')
  })

  it('keeps the newest text when an earlier autosave is still on the wire', async () => {
    let landFirstSave = () => {}
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.saveDraft' && calls.filter((call) => call.command === 'mail.saveDraft').length === 1) {
        await new Promise<void>((resolve) => (landFirstSave = resolve))
      }
      if (command === 'mail.allocateIdentity') return { message_id: 'draft-allocated@example.com' }
      return {}
    }
    const { getByTestId } = render(<ReplyBox />)
    await act(async () => {
      await Promise.resolve()
      compose$.composer.set('First words')
      await new Promise((resolve) => setTimeout(resolve, 1_300))
    })
    await act(async () => {
      compose$.composer.set('First words, then more')
    })

    await act(async () => {
      fireEvent.keyDown(getByTestId('reply'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 0))
      landFirstSave()
      await new Promise((resolve) => setTimeout(resolve, 10))
    })

    expect(kanban$.paneThreadId.get()).toBe('')
    const saves = calls.filter((call) => call.command === 'mail.saveDraft')
    expect(JSON.stringify(saves.at(-1)?.payload)).toContain('First words, then more')
  })

  it('keeps text an autosave queued behind one still on the wire had yet to read', async () => {
    let landFirstSave = () => {}
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.saveDraft' && calls.filter((call) => call.command === 'mail.saveDraft').length === 1) {
        await new Promise<void>((resolve) => (landFirstSave = resolve))
      }
      if (command === 'mail.allocateIdentity') return { message_id: 'draft-allocated@example.com' }
      return {}
    }
    const { getByTestId } = render(<ReplyBox />)
    await act(async () => {
      await Promise.resolve()
      compose$.composer.set('First words')
      await new Promise((resolve) => setTimeout(resolve, 1_300))
    })
    // The second autosave fires too, and waits behind the first.
    await act(async () => {
      compose$.composer.set('First words, then more')
      await new Promise((resolve) => setTimeout(resolve, 1_300))
    })

    await act(async () => {
      fireEvent.keyDown(getByTestId('reply'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 0))
      landFirstSave()
      await new Promise((resolve) => setTimeout(resolve, 10))
    })

    expect(kanban$.paneThreadId.get()).toBe('')
    const saves = calls.filter((call) => call.command === 'mail.saveDraft')
    expect(JSON.stringify(saves.at(-1)?.payload)).toContain('First words, then more')
  })

  it('keeps the saved reply when the card reopens before the save lands', async () => {
    let landSave = () => {}
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) => {
      calls.push({ command, payload })
      if (command === 'mail.saveDraft') await new Promise<void>((resolve) => (landSave = resolve))
      if (command === 'mail.allocateIdentity') return { message_id: 'draft-allocated@example.com' }
      return {}
    }
    const { getByTestId } = render(<ReplyBox />)
    await act(async () => {
      await Promise.resolve()
      compose$.composer.set('Keep me')
    })
    await act(async () => {
      fireEvent.keyDown(getByTestId('reply'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 0))
    })
    expect(kanban$.paneThreadId.get()).toBe('')

    // Enter on the card Esc left the keyboard on, while the save is on the wire.
    await act(async () => {
      ui$.selectedThread.set('t-1')
      kanban$.paneThreadId.set('t-1')
      await new Promise((resolve) => setTimeout(resolve, 0))
      landSave()
      await new Promise((resolve) => setTimeout(resolve, 10))
    })

    expect(JSON.stringify(calls.find((call) => call.command === 'mail.saveDraft')?.payload)).toContain('Keep me')
    expect(calls.some((call) => call.command === 'mail.discardDraft' || call.command === 'mail.delete')).toBe(false)
  })

  it("hands focus to the closed conversation's card", async () => {
    const view = render(
      <>
        <section data-thread-list data-kanban-column="board-1:acc-1:INBOX">
          <button data-thread-row data-active>
            opened card
          </button>
        </section>
        <ReplyBox />
      </>,
    )
    view.getByTestId('reply').focus()
    await act(async () => {
      fireEvent.keyDown(view.getByTestId('reply'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 0))
    })

    expect(kanban$.paneThreadId.get()).toBe('')
    expect(document.activeElement).toBe(view.getByText('opened card'))
  })

  it('closes an empty box without saving a draft', async () => {
    const { getByTestId } = render(<ReplyBox />)
    await act(async () => {
      fireEvent.keyDown(getByTestId('reply'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 0))
    })

    expect(kanban$.paneThreadId.get()).toBe('')
    expect(calls.some((call) => call.command === 'mail.saveDraft')).toBe(false)
  })

  it('leaves Escape alone outside a kanban board', async () => {
    kanban$.activeBoardId.set('')
    kanban$.paneThreadId.set('')
    const { getByTestId } = render(<ReplyBox />)
    await act(async () => {
      fireEvent.keyDown(getByTestId('reply'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 0))
    })

    expect(ui$.selectedThread.get()).toBe('t-1')
  })

  it('leaves Escape alone while a tab is in front of the card conversation', async () => {
    compose$.tabs.set([{ id: 'tab-1', kind: 'thread', threadId: 't-1' } as any])
    compose$.activeTab.set('tab-1')
    const { getByTestId } = render(<ReplyBox />)
    await act(async () => {
      fireEvent.keyDown(getByTestId('reply'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 0))
    })

    expect(kanban$.paneThreadId.get()).toBe('t-1')
  })
})
