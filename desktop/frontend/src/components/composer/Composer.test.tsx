import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, render } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { settings$ } from '../../states/settings'
import { closeMessageTab, compose$, openComposeTab, updateComposeDraft } from '../../states/compose'
import { ui$ } from '../../states/ui'
import type { Account } from '../../types'
import { Composer } from './Composer'
import { ConversationTabs } from '../chat/ConversationTabs'
import { AppConfirm } from '../dialog/AppConfirm'

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
  let discardFailure = false

  beforeEach(() => {
    calls = []
    allocations = 0
    holdAllocation = null
    discardFailure = false
    compose$.tabs.set([])
    compose$.activeTab.set('')
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
            if (command === 'mail.discardDraft' && discardFailure) throw new Error('server refused discard')
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
      expect(draftOf(tabId)).toBeDefined()
      holdAllocation?.()
      holdAllocation = null
      await new Promise((resolve) => setTimeout(resolve, 50))
    })

    const order = calls.map((call) => call.command)
    expect(order.lastIndexOf('mail.discardDraft')).toBeGreaterThan(order.lastIndexOf('mail.saveDraft'))
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

  it('closes an explicitly discarded composer after reporting a discard failure', async () => {
    const tabId = openComposeTab({ to: 'x@example.com', subject: 'Hello', text: 'hi' })!
    const view = render(<Composer tabId={tabId} />)

    await act(async () => {
      updateComposeDraft(tabId, { fromEmail: 'a@example.com' })
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    discardFailure = true
    await act(async () => {
      const discard = [...view.container.querySelectorAll('button')].find((button) => button.textContent === 'Discard')!
      discard.click()
      await new Promise((resolve) => setTimeout(resolve, 20))
    })

    expect(draftOf(tabId)).toBeUndefined()
    expect(ui$.toast.peek()).toBe('Could not discard draft: server refused discard')
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
})
