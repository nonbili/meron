import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { draftOpenInComposeTab } from '../../states/quickReply'
import { settings$ } from '../../states/settings'
import {
  closeMessageTab,
  forwardMessage,
  openComposeTab,
  openDraftCompose,
  updateComposeDraft,
} from '../../states/compose'
import { compose$ } from '../../states/composeState'
import { kanban$ } from '../../states/kanban'
import { dismissToast, runToastUndo, settleConfirm, ui$ } from '../../states/ui'
import type { Account, ComposeDraft, Message } from '../../types'
import { Composer } from './Composer'
import { ConversationTabs } from '../chat/ConversationTabs'
import { AppConfirm } from '../dialog/AppConfirm'
import { AppHotkeys } from '../dialog/AppHotkeys'

const account = (id: string, signatureHtml: string): Account => ({
  id,
  email: `${id}@example.com`,
  display_name: id,
  provider: 'custom',
  auth_type: 'password',
  imap_host: 'imap.example.com',
  imap_port: 993,
  smtp_host: 'smtp.example.com',
  smtp_port: 465,
  tls: true,
  signature: { mode: 'custom', html: signatureHtml },
})

type Call = { command: string; payload: any }

describe('Composer', () => {
  let calls: Call[] = []
  let allocations = 0
  // Set to hold the next allocation open, so a second save can be started while
  // the first is still in flight.
  let holdAllocation: (() => void) | null = null
  let holdSave: (() => void) | null = null
  let holdAttachment: (() => void) | null = null
  let discardFailure = false
  let saveFailure = false
  let pendingDiscard: Promise<void> | null = null

  beforeEach(() => {
    calls = []
    allocations = 0
    holdAllocation = null
    holdSave = null
    holdAttachment = null
    discardFailure = false
    saveFailure = false
    pendingDiscard = null
    dismissToast()
    compose$.tabs.set([])
    compose$.activeTab.set('')
    // Other files can leave a conversation behind the tabs, which adds the
    // Current tab and a second "Close tab" button to the strip.
    compose$.conversationThread.set('')
    kanban$.activeBoardId.set('')
    kanban$.paneThreadId.set('')
    settings$.signature.set('')
    ui$.selectedAccount.set('a')
    accounts$.set([account('a', '<p>From A</p>'), account('b', '<p>From B</p>')])
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            if (command === 'mail.allocateIdentity') {
              allocations += 1
              const id = `allocated-${allocations}@example.com`
              if (holdAllocation) {
                await new Promise<void>((resolve) => {
                  holdAllocation = resolve
                })
              }
              return { message_id: id }
            }
            if (command === 'mail.readAttachment') {
              if (holdAttachment) {
                await new Promise<void>((resolve) => {
                  holdAttachment = resolve
                })
              }
              return { data: 'AAAA', mime: 'application/pdf', size: 3 }
            }
            if (command === 'mail.saveDraft' && holdSave) {
              await new Promise<void>((resolve) => {
                holdSave = resolve
              })
            }
            if (command === 'mail.saveDraft' && saveFailure) throw new Error('server refused save')
            if (command === 'mail.discardDraft') {
              await pendingDiscard
              if (discardFailure) throw new Error('server refused discard')
            }
            return {}
          },
        },
      },
    }
  })

  afterEach(cleanup)

  const draftOf = (tabId: string) => compose$.tabs.peek().find((tab) => tab.id === tabId)?.compose
  const bodyHtml = () => document.querySelector('.tiptap-body')?.innerHTML ?? ''
  const savedDraftIds = () => calls.filter((c) => c.command === 'mail.saveDraft').map((c) => c.payload.draft_id)

  it('scales the plain-text body with the message text size', () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    const view = render(<Composer tabId={tabId} />)

    const body = view.container.querySelector('textarea')
    expect(body?.className).toContain('var(--me-message-scale)')
  })

  it('explains why a reply to unconfirmed decrypted mail has no quote', () => {
    const withheld = openComposeTab({ to: 'x@example.com', subject: 'Re: Hi', rich: false, quoteWithheld: true })!
    const view = render(<Composer tabId={withheld} />)
    expect(view.getByText(/isn't quoted/)).toBeTruthy()
    cleanup()

    const plain = openComposeTab({ to: 'x@example.com', subject: 'Re: Hi', rich: false })!
    expect(render(<Composer tabId={plain} />).queryByText(/isn't quoted/)).toBeNull()
  })

  it('notes a changed recipient key and asks before sending to it', async () => {
    const base = (window as any).go.main.App.Invoke
    ;(window as any).go.main.App.Invoke = async (command: string, payload: unknown) =>
      command === 'pgp.keys.forRecipients'
        ? (calls.push({ command, payload }),
          {
            enabled: true,
            own_key_unusable: false,
            missing: [],
            changed: [{ addr: 'x@example.com', previous: 'A', current: 'B' }],
          })
        : base(command, payload)
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hi', text: 'hi', rich: false, encrypt: true })!
    const view = render(<Composer tabId={tabId} />)
    expect(await view.findByText(/x@example.com's key changed since you last wrote/)).toBeTruthy()

    await act(async () => {
      ;[...view.container.querySelectorAll('button')].find((b) => b.textContent?.includes('Send'))!.click()
    })
    await waitFor(() => expect(ui$.confirm.get()?.title).toBe('Send to a changed key?'))
    await act(async () => {
      settleConfirm(false)
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    expect(calls.some((c) => c.command === 'mail.send')).toBe(false)
    expect(draftOf(tabId)).toBeTruthy()
  })

  it('assigns scrolling and padding to the shrinkable plain-text body', () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    const view = render(<Composer tabId={tabId} />)

    // These assertions check the layout contract, not browser geometry.
    // The textarea owns scrolling; its flex wrapper must not scroll too.
    const body = view.container.querySelector('textarea')!
    const classes = body.className.split(' ')
    expect(classes).toContain('block')
    expect(classes).toContain('px-4')
    expect(classes).toContain('py-3')
    expect(classes).toContain('min-h-0')
    expect(classes).toContain('flex-1')
    expect(classes).not.toContain('min-h-[240px]')
    expect(classes).not.toContain('h-full')
    const wrapperClasses = body.parentElement!.className.split(' ')
    expect(wrapperClasses).toContain('flex')
    expect(wrapperClasses).toContain('flex-col')
    expect(wrapperClasses).toContain('min-h-0')
    expect(wrapperClasses).toContain('overflow-hidden')
    expect(wrapperClasses).not.toContain('overflow-y-auto')
    expect(body.parentElement!.className.split(' ')).not.toContain('px-4')
    expect(body.parentElement!.className.split(' ')).not.toContain('py-3')
  })

  it('keeps the side padding on the scrolling body above a folded quote', () => {
    const quote = { rich: false, html: '<p>On Mon, Them wrote:</p>', text: 'On Mon, Them wrote:\n> Lunch?' }
    const tabId = openComposeTab({ to: 'them@example.com', rich: false, text: 'Sounds good', quote })!
    const view = render(<Composer tabId={tabId} />)

    const body = view.container.querySelector('textarea')!
    expect(body.className.split(' ')).toContain('block')
    expect(body.className.split(' ')).not.toContain('px-4')
    expect(body.className.split(' ')).not.toContain('py-3')
    expect(body.parentElement!.className.split(' ')).toContain('px-4')
    expect(body.parentElement!.className.split(' ')).toContain('py-3')
    expect(body.parentElement!.className.split(' ')).toContain('overflow-y-auto')
  })

  it('scales the rich-text body with the message text size', () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', rich: true })!
    render(<Composer tabId={tabId} />)

    expect(document.querySelector('.tiptap-body')?.className).toContain('var(--me-message-scale)')
  })

  it('allocates one server draft when autosaves overlap', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    render(<Composer tabId={tabId} />)

    // Hold the first allocation open, then start a second save behind it: this
    // is the window in which both used to allocate a draft of their own.
    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'alias@example.com' })
    })
    await act(async () => {
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(allocations).toBe(1)
    const ids = savedDraftIds()
    expect(ids.length).toBeGreaterThan(1)
    // Every save wrote to the same server draft, so none was orphaned.
    expect(new Set(ids).size).toBe(1)
    expect(draftOf(tabId)?.draftMessageId).toBe(ids[0])
  })

  it('keeps one autosave queue when the composer unmounts and remounts during allocation', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const first = render(<Composer tabId={tabId} />)

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    first.unmount()
    render(<Composer tabId={tabId} />)
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'alias@example.com' })
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    const ids = savedDraftIds()
    expect(allocations).toBe(1)
    expect(ids.length).toBeGreaterThan(1)
    expect(new Set(ids).size).toBe(1)
    expect(draftOf(tabId)?.draftMessageId).toBe(ids[0])
  })

  it('never writes the draft back to the server after it has been sent', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const view = render(<Composer tabId={tabId} />)

    // A save is in flight (and another queued behind it) when Send is pressed.
    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'alias@example.com' })
    })

    await act(async () => {
      const send = [...view.container.querySelectorAll('button')].find((b) => b.textContent?.includes('Send'))!
      send.click()
      // The send waits for the queue; releasing it lets everything settle.
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    const order = calls.map((call) => call.command)
    const sent = order.indexOf('mail.send')
    const discarded = order.lastIndexOf('mail.discardDraft')
    expect(sent).toBeGreaterThan(-1)
    expect(discarded).toBeGreaterThan(sent)
    // Nothing may re-save the draft after it was discarded, or the sent message
    // reappears in Drafts.
    expect(order.slice(discarded).includes('mail.saveDraft')).toBe(false)
    // And the draft discarded is the one the saves actually created.
    const discardPayload = calls.filter((c) => c.command === 'mail.discardDraft').pop()!.payload
    expect(discardPayload.draft_id).toBe(savedDraftIds()[0])
  })

  it('saves the account and body as they stand when the queued save runs', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    render(<Composer tabId={tabId} />)

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    // Queued while the draft still belongs to account 'a'…
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'alias@example.com' })
    })
    // …but by the time it runs, the user has moved the draft to 'b'.
    await act(async () => {
      updateComposeDraft(tabId, { accountId: 'b', text: 'moved to b' })
    })
    await act(async () => {
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    const saves = calls.filter((c) => c.command === 'mail.saveDraft').map((c) => c.payload)
    // Only the save already in flight when the switch happened may carry 'a';
    // every later one is 'b', with the body that goes with it.
    expect(saves.filter((s) => s.account_id === 'a')).toHaveLength(1)
    const last = saves[saves.length - 1]
    expect(last.account_id).toBe('b')
    expect(last.body).toContain('moved to b')
  })

  it('does not attach an in-flight allocation to a newly selected account', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    render(<Composer tabId={tabId} />)

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      updateComposeDraft(tabId, { accountId: 'b', fromEmail: 'b@example.com' })
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    const saves = calls.filter((call) => call.command === 'mail.saveDraft').map((call) => call.payload)
    const allocatedForA = saves.find((save) => save.account_id === 'a')!.draft_id
    const savedForB = saves.filter((save) => save.account_id === 'b').pop()!
    expect(savedForB.draft_id).not.toBe(allocatedForA)
    expect(draftOf(tabId)?.draftMessageId).toBe(savedForB.draft_id)
    expect(
      calls.some(
        (call) =>
          call.command === 'mail.discardDraft' &&
          call.payload.account_id === 'a' &&
          call.payload.draft_id === allocatedForA,
      ),
    ).toBe(true)
  })

  it('deletes the server draft when the composer is discarded mid-save', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const view = render(<Composer tabId={tabId} />)

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })

    await act(async () => {
      const discard = [...view.container.querySelectorAll('button')].find((b) => b.textContent === 'Discard')!
      discard.click()
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    const order = calls.map((call) => call.command)
    const discarded = order.lastIndexOf('mail.discardDraft')
    expect(discarded).toBeGreaterThan(-1)
    // The draft deleted is the one the save created, and nothing re-saves it.
    expect(calls[discarded].payload.draft_id).toBe(savedDraftIds()[0])
    expect(order.slice(discarded).includes('mail.saveDraft')).toBe(false)
  })

  it('drains and discards when the tab strip closes the composer', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const view = render(
      <>
        <ConversationTabs />
        <Composer tabId={tabId} />
      </>,
    )

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      view.getByTitle('Close tab').click()
      expect(draftOf(tabId)).toBeUndefined()
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    const order = calls.map((call) => call.command)
    expect(order.lastIndexOf('mail.discardDraft')).toBeGreaterThan(order.lastIndexOf('mail.saveDraft'))
    expect(draftOf(tabId)).toBeUndefined()
  })

  it('discards the copy a failed save may have written after the tab closed', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const view = render(
      <>
        <ConversationTabs />
        <Composer tabId={tabId} />
      </>,
    )

    holdAllocation = () => {}
    saveFailure = true
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      view.getByTitle('Close tab').click()
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(draftOf(tabId)).toBeUndefined()
    const discarded = calls.filter((call) => call.command === 'mail.discardDraft')
    expect(discarded.map((call) => call.payload.draft_id)).toContain('allocated-1@example.com')
  })

  it('closes on Esc at once and keeps the message in Drafts', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    compose$.activeTab.set(tabId)
    const view = render(<Composer tabId={tabId} />)

    // A slow Drafts server: the tab must not wait for it.
    holdAllocation = () => {}
    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Hello'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    expect(draftOf(tabId)).toBeUndefined()

    await act(async () => {
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })
    // Saved without waiting for the autosave debounce, and never discarded.
    expect(savedDraftIds()).toEqual(['allocated-1@example.com'])
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)
  })

  it('closes on Esc from the rich-text body, which marks the key handled', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', html: '<p>hi</p>', rich: true })!
    compose$.activeTab.set(tabId)
    render(<Composer tabId={tabId} />)

    // ProseMirror calls preventDefault on Esc (keyCode 27) in the editor.
    await act(async () => {
      fireEvent.keyDown(document.querySelector('.tiptap-body')!, { key: 'Escape', keyCode: 27 })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(draftOf(tabId)).toBeUndefined()
    expect(savedDraftIds()).toEqual(['allocated-1@example.com'])
  })

  it('keeps the draft when Esc comes while an autosave allocates it', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    const view = render(<Composer tabId={tabId} />)

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Hello'), { key: 'Escape' })
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    // The autosave cleans up the copy it allocated for the closed tab; the
    // save on Esc, which runs after it, writes the one that stays.
    const order = calls.map((call) => call.command)
    expect(order.lastIndexOf('mail.saveDraft')).toBeGreaterThan(order.lastIndexOf('mail.discardDraft'))
    expect(savedDraftIds().at(-1)).toBe('allocated-2@example.com')
  })

  it('brings the tab back when the save on Esc fails', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    compose$.activeTab.set(tabId)
    const view = render(<Composer tabId={tabId} />)

    saveFailure = true
    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Hello'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(draftOf(tabId)?.text).toStartWith('hi')
    expect(compose$.activeTab.peek()).toBe(tabId)
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)
  })

  it('keeps the edits of a failed save when the draft was reopened meanwhile', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'latest', rich: false })!
    const view = render(<Composer tabId={tabId} />)

    saveFailure = true
    holdAllocation = () => {}
    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Hello'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    // The older copy, opened from Drafts while the save is still running.
    const older = openComposeTab({ draftMessageId: 'allocated-1@example.com', text: 'older', rich: false })!
    await act(async () => {
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(draftOf(tabId)?.text).toStartWith('latest')
    expect(draftOf(tabId)?.draftMessageId).toStartWith('local-draft-')
    expect(draftOf(older)?.draftMessageId).toBe('allocated-1@example.com')
    expect(compose$.activeTab.peek()).toBe(tabId)
  })

  const draftRow = (messageId = 'saved@example.com') =>
    ({
      id: `a#Drafts#${messageId}`,
      account_id: 'a',
      thread_id: `a#Drafts#t-${messageId}`,
      folder_id: 'Drafts',
      message_id: messageId,
      subject: 'Hello',
      body: 'older',
    }) as unknown as Message

  const openSavedDraftTab = () => {
    const tabId = openComposeTab({
      to: 'x@example.com',
      subject: 'Hello',
      text: 'latest',
      rich: false,
      draftMessageId: 'saved@example.com',
    })!
    return { tabId, view: render(<Composer tabId={tabId} />) }
  }

  const pressEscWithSaveHeld = async (input: HTMLElement) => {
    holdSave = () => {}
    await act(async () => {
      fireEvent.keyDown(input, { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
  }

  const releaseSave = () => {
    holdSave?.()
    holdSave = null
  }

  const escThenOpenRow = async (input: HTMLElement) => {
    await pressEscWithSaveHeld(input)
    const opening = openDraftCompose(draftRow())
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    // Still waiting for the save: nothing opened from the older row.
    expect(compose$.tabs.peek()).toHaveLength(0)
    await act(async () => {
      releaseSave()
      await opening
    })
  }

  it('opens a draft from its Esc save, not the older row it replaces', async () => {
    const { tabId, view } = openSavedDraftTab()

    await escThenOpenRow(view.getByDisplayValue('Hello'))

    expect(compose$.tabs.peek().map((tab) => tab.id)).toEqual([tabId])
    expect(draftOf(tabId)?.text).toStartWith('latest')
    expect(compose$.activeTab.peek()).toBe(tabId)
    expect(calls.some((call) => call.command === 'mail.threadRead')).toBe(false)
  })

  it('opens the restored tab when the Esc save fails while a draft row opens', async () => {
    const { tabId, view } = openSavedDraftTab()
    saveFailure = true

    await escThenOpenRow(view.getByDisplayValue('Hello'))

    expect(compose$.tabs.peek().map((tab) => tab.id)).toEqual([tabId])
    expect(draftOf(tabId)?.text).toStartWith('latest')
    expect(compose$.activeTab.peek()).toBe(tabId)
  })

  it('counts a draft as open in a tab until its Esc save lands', async () => {
    const { view } = openSavedDraftTab()
    await pressEscWithSaveHeld(view.getByDisplayValue('Hello'))

    expect(draftOpenInComposeTab('saved@example.com')).toBe(true)

    await act(async () => {
      releaseSave()
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    expect(draftOpenInComposeTab('saved@example.com')).toBe(false)
  })

  it('opens another draft without waiting for an Esc save', async () => {
    const { view } = openSavedDraftTab()
    await pressEscWithSaveHeld(view.getByDisplayValue('Hello'))

    await act(async () => {
      await openDraftCompose(draftRow('other@example.com'))
    })

    expect(calls.some((call) => call.command === 'mail.threadRead')).toBe(true)
    expect(compose$.tabs.peek().map((tab) => tab.compose?.draftMessageId)).toEqual(['other@example.com'])

    await act(async () => {
      releaseSave()
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
  })

  const forwardWithAttachmentHeld = async () => {
    holdAttachment = () => {}
    void forwardMessage({
      id: 'a#INBOX#1#1',
      account_id: 'a',
      subject: 'Report',
      body: 'see attached',
      attachments: [{ key: 'k1', filename: 'report.pdf', mime: 'application/pdf', size: 3 }],
    } as unknown as Message)
    const tabId = compose$.activeTab.peek()
    const view = render(<Composer tabId={tabId} />)
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    return { tabId, view }
  }

  const savedAttachmentNames = () =>
    calls.filter((c) => c.command === 'mail.saveDraft').map((c) => c.payload.attachments.map((a: any) => a.filename))

  it('waits for attachments still on their way before the save on Esc', async () => {
    const { tabId, view } = await forwardWithAttachmentHeld()

    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Fwd: Report'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    // Closed at once, with nothing saved without the attachment.
    expect(draftOf(tabId)).toBeUndefined()
    expect(savedAttachmentNames()).toEqual([])

    await act(async () => {
      holdAttachment?.()
      await new Promise((resolve) => setTimeout(resolve, 50))
    })
    expect(savedAttachmentNames()).toEqual([['report.pdf']])

    runToastUndo()
    expect(draftOf(tabId)?.attachments.map((a) => a.filename)).toEqual(['report.pdf'])
  })

  it('brings attachments back with the tab when the save on Esc fails', async () => {
    const { tabId, view } = await forwardWithAttachmentHeld()
    saveFailure = true

    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Fwd: Report'), { key: 'Escape' })
      holdAttachment?.()
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(draftOf(tabId)?.attachments.map((a) => a.filename)).toEqual(['report.pdf'])
  })

  it('closes the compose tab in front on Esc from outside it, not the pane behind', async () => {
    kanban$.activeBoardId.set('board')
    kanban$.paneThreadId.set('a#INBOX#1')
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    compose$.activeTab.set(tabId)
    render(
      <>
        <AppHotkeys />
        <Composer tabId={tabId} />
      </>,
    )

    await act(async () => {
      fireEvent.keyDown(document.body, { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(savedDraftIds()).toEqual(['allocated-1@example.com'])
    expect(draftOf(tabId)).toBeUndefined()
    expect(kanban$.paneThreadId.peek()).toBe('a#INBOX#1')
  })

  it('leaves Esc to an IME composing text', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    const view = render(<Composer tabId={tabId} />)

    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Hello'), { key: 'Escape', isComposing: true })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(draftOf(tabId)).toBeDefined()
  })

  it('leaves to an IME the Esc WebKit reports as keyCode 229', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    const view = render(<Composer tabId={tabId} />)

    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Hello'), { key: 'Escape', keyCode: 229 })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(draftOf(tabId)).toBeDefined()
  })

  it('keeps what was typed before the tab was switched away on Esc', async () => {
    const tabId = openComposeTab({ rich: false })!
    const first = render(<Composer tabId={tabId} />)
    await act(async () => {
      updateComposeDraft(tabId, { text: 'written, then the tab was left' })
    })
    // An inactive tab is unmounted, so coming back mounts a new composer.
    first.unmount()
    const view = render(<Composer tabId={tabId} />)

    await act(async () => {
      fireEvent.keyDown(view.container.querySelector('textarea')!, { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(savedDraftIds()).toEqual(['allocated-1@example.com'])
    expect(calls.some((call) => call.command === 'mail.discardDraft')).toBe(false)
  })

  it('keeps a body seeded with no recipient or subject on Esc', async () => {
    const tabId = openComposeTab({ text: 'from a mailto body' })!
    const view = render(<Composer tabId={tabId} />)

    await act(async () => {
      fireEvent.keyDown(view.container.querySelector('textarea')!, { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(savedDraftIds()).toEqual(['allocated-1@example.com'])
  })

  it('says the message went to Drafts and reopens it from the toast', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    compose$.activeTab.set(tabId)
    const view = render(<Composer tabId={tabId} />)

    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Hello'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })
    expect(ui$.toast.peek()).toBe('Saved to Drafts')
    expect(ui$.toastActionLabel.peek()).toBe('Open')

    runToastUndo()

    // Under the saved id, so the next save replaces that copy.
    expect(draftOf(tabId)?.draftMessageId).toBe('allocated-1@example.com')
    expect(draftOf(tabId)?.text).toStartWith('hi')
    expect(compose$.activeTab.peek()).toBe(tabId)
  })

  it('goes to the tab already holding the draft when opened from the toast', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi', rich: false })!
    const view = render(<Composer tabId={tabId} />)

    await act(async () => {
      fireEvent.keyDown(view.getByDisplayValue('Hello'), { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })
    const fromDrafts = openComposeTab({ draftMessageId: 'allocated-1@example.com', text: 'hi', rich: false })!
    compose$.activeTab.set('')

    runToastUndo()

    expect(compose$.tabs.peek().map((tab) => tab.id)).toEqual([fromDrafts])
    expect(compose$.activeTab.peek()).toBe(fromDrafts)
  })

  const pressEscIn = async (tabId: string, body: Partial<ComposeDraft>) => {
    await act(async () => {
      updateComposeDraft(tabId, body)
    })
    await act(async () => {
      fireEvent.keyDown(document.querySelector('.tiptap-body, textarea')!, { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })
  }

  it('throws away a body typed in and emptied again on Esc', async () => {
    const tabId = openComposeTab({ rich: true })!
    render(<Composer tabId={tabId} />)

    // What the editor reports once everything typed above the signature is gone.
    await pressEscIn(tabId, { html: '<p></p><p></p><p>From A</p>' })

    expect(draftOf(tabId)).toBeUndefined()
    expect(savedDraftIds()).toEqual([])
  })

  it('throws away a body of only white space on Esc', async () => {
    const tabId = openComposeTab({ rich: false })!
    render(<Composer tabId={tabId} />)

    await pressEscIn(tabId, { text: '  \n \n\nFrom A' })

    expect(savedDraftIds()).toEqual([])
    expect(ui$.toast.peek()).toBe('')
  })

  it('keeps an image with no text around it on Esc', async () => {
    const tabId = openComposeTab({ rich: true })!
    render(<Composer tabId={tabId} />)

    await pressEscIn(tabId, { html: '<p><img src="https://example.com/a.png"></p><p></p><p>From A</p>' })

    expect(savedDraftIds()).toEqual(['allocated-1@example.com'])
  })

  it('keeps a signature that was edited on Esc', async () => {
    const tabId = openComposeTab({ rich: true })!
    render(<Composer tabId={tabId} />)

    await pressEscIn(tabId, { html: '<p></p><p>From A, and more</p>' })

    expect(savedDraftIds()).toEqual(['allocated-1@example.com'])
  })

  it('throws away a new message nothing was written in on Esc', async () => {
    const tabId = openComposeTab({ rich: false })!
    const view = render(<Composer tabId={tabId} />)

    await act(async () => {
      fireEvent.keyDown(view.container.querySelector('textarea')!, { key: 'Escape' })
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(savedDraftIds()).toEqual([])
    expect(draftOf(tabId)).toBeUndefined()
  })

  it('deduplicates simultaneous footer and external close requests', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const view = render(
      <>
        <ConversationTabs />
        <Composer tabId={tabId} />
      </>,
    )

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      const discard = [...view.container.querySelectorAll('button')].find((button) => button.textContent === 'Discard')!
      discard.click()
      const externalClose = closeMessageTab(tabId)
      holdAllocation?.()
      holdAllocation = null
      await externalClose
      await new Promise((resolve) => setTimeout(resolve, 20))
    })

    expect(calls.filter((call) => call.command === 'mail.discardDraft')).toHaveLength(1)
    expect(draftOf(tabId)).toBeUndefined()
  })

  for (const mounted of [true, false]) {
    it(`closes an empty ${mounted ? 'mounted' : 'unmounted'} composer before remote discard finishes`, async () => {
      const tabId = openComposeTab()!
      updateComposeDraft(tabId, { draftMessageId: 'saved-empty@example.com' })
      if (mounted) render(<Composer tabId={tabId} />)

      let releaseDiscard!: () => void
      pendingDiscard = new Promise<void>((resolve) => {
        releaseDiscard = resolve
      })
      let closing!: Promise<void>
      try {
        await act(async () => {
          closing = closeMessageTab(tabId)
        })
        expect(draftOf(tabId)).toBeUndefined()
        expect(calls.filter((call) => call.command === 'mail.discardDraft')).toHaveLength(1)
        await closeMessageTab(tabId)
        expect(calls.filter((call) => call.command === 'mail.discardDraft')).toHaveLength(1)
      } finally {
        await act(async () => {
          releaseDiscard()
          await closing
        })
      }
    })
  }

  describe('when a discard fails', () => {
    const realSetTimeout = globalThis.setTimeout
    const wait = (ms: number) =>
      act(async () => {
        await new Promise((resolve) => realSetTimeout(resolve, ms))
      })
    const discardWithFailure = async () => {
      const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
      const view = render(<Composer tabId={tabId} />)
      await act(async () => {
        updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
      })
      await wait(20)
      discardFailure = true
      await act(async () => {
        ;[...view.container.querySelectorAll('button')].find((button) => button.textContent === 'Discard')!.click()
      })
      await wait(10)
      return tabId
    }

    beforeEach(() => {
      // The retry's delay, cut short.
      globalThis.setTimeout = ((handler: () => void, ms?: number) =>
        realSetTimeout(handler, ms === 5_000 ? 40 : ms)) as typeof setTimeout
    })
    afterEach(() => {
      globalThis.setTimeout = realSetTimeout
    })

    it('closes the composer and reports it once the retry fails too', async () => {
      const tabId = await discardWithFailure()
      expect(draftOf(tabId)).toBeUndefined()
      // The first failure may only be a slow connection.
      expect(ui$.toast.peek()).toBe('')

      await wait(80)
      expect(ui$.toast.peek()).toBe('Could not discard draft: server refused discard')
    })

    it('says nothing when the retry succeeds', async () => {
      await discardWithFailure()
      discardFailure = false

      await wait(80)
      expect(ui$.toast.peek()).toBe('')
    })
  })

  it('cleans up the old account’s draft when the draft moves accounts', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    render(<Composer tabId={tabId} />)

    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    const allocatedForA = savedDraftIds()[0]
    expect(allocatedForA).toStartWith('allocated-')

    await act(async () => {
      updateComposeDraft(tabId, { accountId: 'b' })
      await new Promise((resolve) => setTimeout(resolve, 20))
    })

    // A's copy is deleted where it actually lives, and the tab stops pointing
    // at an id that belongs to another account's Drafts folder.
    const discards = calls.filter((c) => c.command === 'mail.discardDraft').map((c) => c.payload)
    expect(discards).toHaveLength(1)
    expect(discards[0]).toMatchObject({ account_id: 'a', draft_id: allocatedForA })
    expect(draftOf(tabId)?.draftMessageId).not.toBe(allocatedForA)
  })

  it('sends what is on screen, not what was there when Send was pressed', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'first draft' })!
    const view = render(<Composer tabId={tabId} />)

    // A save is in flight, so Send has to wait for it — and the composer stays
    // editable while it does.
    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })

    await act(async () => {
      const send = [...view.container.querySelectorAll('button')].find((b) => b.textContent?.includes('Send'))!
      send.click()
      updateComposeDraft(tabId, { text: 'second thoughts', to: 'y@example.com' })
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    const sent = calls.find((call) => call.command === 'mail.send')!.payload
    expect(sent.body).toContain('second thoughts')
    expect(sent.to).toBe('y@example.com')
  })

  it('revalidates recipient and account after autosaves drain', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const view = render(<Composer tabId={tabId} />)

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      const send = [...view.container.querySelectorAll('button')].find((button) =>
        button.textContent?.includes('Send'),
      )!
      send.click()
      updateComposeDraft(tabId, { to: '' })
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(calls.some((call) => call.command === 'mail.send')).toBe(false)
    expect(view.container.textContent).toContain('Add a recipient before sending')
  })

  it('revalidates the selected account after autosaves drain', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const view = render(<Composer tabId={tabId} />)

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      const send = [...view.container.querySelectorAll('button')].find((button) =>
        button.textContent?.includes('Send'),
      )!
      send.click()
      updateComposeDraft(tabId, { accountId: 'missing', fromEmail: 'missing@example.com' })
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(calls.some((call) => call.command === 'mail.send')).toBe(false)
    expect(view.container.textContent).toContain('Choose an available mail account')
  })

  it('asks for no-subject confirmation using the post-drain draft', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const view = render(
      <>
        <Composer tabId={tabId} />
        <AppConfirm />
      </>,
    )

    holdAllocation = () => {}
    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
    })
    await act(async () => {
      const send = [...view.container.querySelectorAll('button')].find((button) =>
        button.textContent?.includes('Send'),
      )!
      send.click()
      updateComposeDraft(tabId, { subject: '' })
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    expect(view.getByRole('alertdialog').textContent).toContain('Send this message without a subject?')
    expect(calls.some((call) => call.command === 'mail.send')).toBe(false)
  })

  it('shows the new account’s signature in the editor when From changes', async () => {
    const tabId = openComposeTab()!
    render(<Composer tabId={tabId} />)
    expect(bodyHtml()).toContain('From A')

    await act(async () => {
      updateComposeDraft(tabId, { accountId: 'b', fromEmail: 'b@example.com' })
    })

    // The editor holds its own copy of the document; if it is not re-seeded the
    // message goes out under the previous account's signature.
    expect(bodyHtml()).toContain('From B')
    expect(bodyHtml()).not.toContain('From A')
  })

  it('keeps what the user typed when the signature is swapped under it', async () => {
    const tabId = openComposeTab()!
    render(<Composer tabId={tabId} />)

    await act(async () => {
      updateComposeDraft(tabId, { html: `<p>Hello there</p>${draftOf(tabId)!.html}` })
    })
    await act(async () => {
      updateComposeDraft(tabId, { accountId: 'b' })
    })

    expect(bodyHtml()).toContain('Hello there')
    expect(bodyHtml()).toContain('From B')
  })

  it('starts the caret above a quote seeded without a signature', () => {
    accounts$.set([{ ...account('a', ''), signature: { mode: 'none', html: '' } }])
    const tabId = openComposeTab({
      to: 'them@example.com',
      rich: false,
      text: '\n\n---------- Forwarded message ---------\nLunch?',
      signaturePlacement: 'aboveQuote',
    })!
    const view = render(<Composer tabId={tabId} />)

    const body = view.container.querySelector('textarea') as HTMLTextAreaElement
    expect(body.selectionStart).toBe(0)
  })

  it('unfolds a folded quote into the body, or drops it', async () => {
    accounts$.set([{ ...account('a', ''), signature: { mode: 'none', html: '' } }])
    const quote = { rich: false, html: '<p>On Mon, Them wrote:</p>', text: 'On Mon, Them wrote:\n> Lunch?' }
    const tabId = openComposeTab({ to: 'them@example.com', rich: false, text: 'Sounds good', quote })!
    const view = render(<Composer tabId={tabId} />)

    await act(async () => {
      view.getByTitle('Show quoted text').click()
    })
    expect(draftOf(tabId)?.text).toBe('Sounds good\n\nOn Mon, Them wrote:\n> Lunch?')
    expect(draftOf(tabId)?.quote).toBeUndefined()
    expect(view.queryByTitle('Show quoted text')).toBeNull()

    const other = openComposeTab({ to: 'them@example.com', rich: false, text: 'Sounds good', quote })!
    cleanup()
    const second = render(<Composer tabId={other} />)
    await act(async () => {
      fireEvent.contextMenu(second.getByTitle('Show quoted text'))
    })
    await act(async () => {
      second.getByText('Remove quoted text').click()
    })
    expect(draftOf(other)?.text).toBe('Sounds good')
    expect(draftOf(other)?.quote).toBeUndefined()
  })
})
