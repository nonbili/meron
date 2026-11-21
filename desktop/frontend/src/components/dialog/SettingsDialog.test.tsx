import { afterEach, beforeEach, describe, expect, it } from 'bun:test'
import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import type { Account } from '../../types'
import { ui$, openSettingsAt } from '../../states/ui'
import { settings$ } from '../../states/settings'
import { AccountPanel, SettingsDialog } from './SettingsDialog'

const account = (avatar_url: string): Account =>
  ({
    id: 'a1',
    email: 'a1@example.com',
    display_name: 'A One',
    provider: 'custom',
    auth_type: 'password',
    imap_host: 'imap.example.com',
    imap_port: 993,
    smtp_host: 'smtp.example.com',
    smtp_port: 465,
    tls: true,
    avatar_url,
  }) as Account

type Call = { command: string; payload: any }

describe('AccountPanel avatar', () => {
  let calls: Call[] = []

  beforeEach(() => {
    calls = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            calls.push({ command, payload })
            return {}
          },
        },
      },
    }
  })

  afterEach(cleanup)

  it('removes a custom avatar from the settings header', async () => {
    accounts$.set([account('/media/avatars/a1/pic.png')])
    const view = render(<AccountPanel account={accounts$.peek()[0]} />)

    await act(async () => {
      fireEvent.click(view.getByRole('button', { name: 'Remove avatar' }))
    })

    expect(calls.filter((call) => call.command === 'account.setAvatar').map((call) => call.payload)).toEqual([
      { id: 'a1', avatar_url: '' },
    ])
    expect(accounts$.peek()[0].avatar_url).toBe('')
  })

  it('offers no remove button while the account has no custom avatar', () => {
    accounts$.set([account('')])
    const view = render(<AccountPanel account={accounts$.peek()[0]} />)
    expect(view.queryByRole('button', { name: 'Remove avatar' })).toBeNull()
  })
})

describe('SettingsDialog reveal', () => {
  afterEach(() => {
    cleanup()
    ui$.settingsOpen.set(false)
    ui$.accountSettingsId.set('')
  })

  it('opens the asked-for page and focuses the revealed row', async () => {
    ;(window as any).go = { main: { App: { Invoke: async () => ({}) } } }
    openSettingsAt('composer', 'Spell check')
    const view = render(<SettingsDialog />)
    const toggle = view.getByRole('switch', { name: 'Spell check' })
    await act(() => new Promise((resolve) => requestAnimationFrame(() => resolve(undefined))))
    expect(document.activeElement).toBe(toggle)
    expect(toggle.closest('[data-setting-title]')?.classList.contains('setting-reveal')).toBe(true)
    expect(ui$.settingsReveal.peek()).toBe('')
  })

  it('focuses the editor of a group made of more than setting rows', async () => {
    ;(window as any).go = { main: { App: { Invoke: async () => ({}) } } }
    openSettingsAt('composer', 'Signature')
    render(<SettingsDialog />)
    await act(() => new Promise((resolve) => setTimeout(resolve, 50)))
    await act(() => new Promise((resolve) => requestAnimationFrame(() => resolve(undefined))))
    const editor = document.querySelector('[data-setting-title="Signature"] [contenteditable="true"]')
    expect(editor).not.toBeNull()
    expect(document.activeElement).toBe(editor)
  })

  it("focuses a number row's field rather than its Reset button", async () => {
    ;(window as any).go = { main: { App: { Invoke: async () => ({}) } } }
    const previousScale = settings$.fontScale.peek()
    settings$.fontScale.set(1.2)
    openSettingsAt('general', 'Text size')
    const view = render(<SettingsDialog />)
    await act(() => new Promise((resolve) => requestAnimationFrame(() => resolve(undefined))))
    const row = view.container.ownerDocument.querySelector('[data-setting-title="Text size"]')!
    expect(row.querySelector('[data-setting-control] button')).not.toBeNull()
    expect(document.activeElement).toBe(row.querySelector('[data-setting-control] input'))
    settings$.fontScale.set(previousScale)
  })

  it("reveals a row on an account's own page", async () => {
    ;(window as any).go = { main: { App: { Invoke: async () => ({}) } } }
    accounts$.set([account('')])
    openSettingsAt('a1', 'Load remote images')
    const view = render(<SettingsDialog />)
    const toggle = view.getByRole('switch', { name: 'Load remote images' })
    await act(() => new Promise((resolve) => requestAnimationFrame(() => resolve(undefined))))
    expect(document.activeElement).toBe(toggle)
  })
})

describe('SettingsDialog remote senders', () => {
  afterEach(() => {
    cleanup()
    ui$.settingsOpen.set(false)
    settings$.remoteImageSenders.set([])
  })

  it('adds a normalized address from an empty allowlist and prevents duplicates', () => {
    settings$.remoteImageSenders.set([])
    openSettingsAt('messages', '')
    const view = render(<SettingsDialog />)
    const row = document.querySelector('[data-setting-title="Always load remote content"]')!
    fireEvent.click(row.querySelector('button')!)

    const address = view.getByRole('textbox', { name: 'Email Address' })
    const add = view.getByRole('button', { name: 'Add' })
    fireEvent.change(address, { target: { value: 'newsletter' } })
    expect(add.hasAttribute('disabled')).toBe(true)

    fireEvent.change(address, { target: { value: ' Newsletter <News@Example.COM> ' } })
    fireEvent.click(add)
    expect(view.getByText('news@example.com')).toBeTruthy()
    expect(settings$.remoteImageSenders.peek()).toEqual(['news@example.com'])

    fireEvent.change(address, { target: { value: 'NEWS@example.com' } })
    expect(add.hasAttribute('disabled')).toBe(true)
    fireEvent.click(view.getByRole('button', { name: 'Stop allowing this sender' }))
    expect(add.hasAttribute('disabled')).toBe(false)
  })
})
