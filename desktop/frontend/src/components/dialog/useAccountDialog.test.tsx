import { afterEach, beforeEach, describe, expect, it, jest } from 'bun:test'
import { act, cleanup, fireEvent, render, renderHook, screen } from '@testing-library/react'
import { useValue } from '@legendapp/state/react'
import { accounts$ } from '../../states/accounts'
import { connectivity$, dismissSyncError, setSyncError } from '../../states/connectivity'
import { ui$ } from '../../states/ui'
import { settings$ } from '../../states/settings'
import type { Account, SystemCheck } from '../../types'
import { AccountDialog } from './AccountDialog'
import { GoogleCalendarSetupDialog } from './GoogleCalendarSetupDialog'
import { useAccountDialog } from './useAccountDialog'

const account: Account = {
  id: 'legacy-user1-mail-localhost',
  email: 'user1@mail.localhost',
  display_name: 'Elena Rostova',
  provider: 'custom',
  auth_type: 'password',
  imap_host: '127.0.0.1',
  imap_port: 1143,
  smtp_host: '127.0.0.1',
  smtp_port: 1025,
  tls: false,
  needs_reconnect: true,
}

describe('account setup, reconnect and edit', () => {
  let saved: any
  let fail: boolean
  let previousGo: PropertyDescriptor | undefined
  let profileEmail: string
  let listedAccounts: Account[]
  let calendarRequests: unknown[]
  let rejectCalendar: boolean

  beforeEach(() => {
    saved = null
    fail = false
    profileEmail = account.email
    listedAccounts = [{ ...account, needs_reconnect: false }]
    calendarRequests = []
    rejectCalendar = false
    previousGo = Object.getOwnPropertyDescriptor(window, 'go')
    accounts$.set([account])
    ui$.setupMode.set('custom')
    ui$.reconnectAccountId.set(account.id)
    ui$.googleCalendarSetupEmail.set('')
    settings$.calendarEnabled.set(true)
    ui$.setupOpen.set(true)
    ui$.settingsOpen.set(false)
    ui$.system.set(null)
    setSyncError(account.id, 'old connection failure')
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: any) => {
            if (
              command === 'account.addPassword' ||
              command === 'account.addGmailOAuth' ||
              command === 'account.addOutlookOAuth'
            ) {
              saved = payload
              if (fail) throw new Error('reconnect failed')
              return { account: { id: payload.id ?? payload.email } }
            }
            if (command === 'oauth.gmailBegin' || command === 'oauth.outlookBegin')
              return { url: 'https://example.com/oauth', needs_external_browser: true }
            if (command === 'oauth.gmailPollProfile' || command === 'oauth.outlookPollProfile')
              return {
                exchanged: true,
                profile: {
                  email: profileEmail,
                  display_name: 'User',
                  access_token: 'access',
                  refresh_token: 'refresh',
                  expires_in: 3600,
                },
              }
            if (command === 'account.list') return { accounts: listedAccounts }
            if (command === 'caldav.connectGoogle') {
              calendarRequests.push(payload)
              if (rejectCalendar) throw new Error('Sign-in was declined')
              return { accounts: [] }
            }
            if (command === 'app.prefsGet') return { prefs: {} }
            if (command === 'mailto.consumePending') return []
            return {}
          },
        },
      },
    }
  })

  afterEach(() => {
    cleanup()
    jest.useRealTimers()
    dismissSyncError()
    ui$.reconnectAccountId.set('')
    ui$.googleCalendarSetupEmail.set('')
    if (previousGo) Object.defineProperty(window, 'go', previousGo)
    else delete (window as any).go
  })

  async function reconnect() {
    const hook = renderHook(() => useAccountDialog())
    await act(async () => hook.result.current.setForm((form) => ({ ...form, password: 'new-password' })))
    await act(async () => {
      await hook.result.current.save()
    })
    return hook
  }

  it('retains the original account ID and clears its banner', async () => {
    await reconnect()
    expect(saved.id).toBe(account.id)
    expect(accounts$.peek().map((entry) => entry.id)).toEqual([account.id])
    expect(connectivity$.error.peek()).toBeNull()
  })

  it('keeps the banner when reconnect fails', async () => {
    fail = true
    const hook = await reconnect()
    expect(hook.result.current.error).toBe('reconnect failed')
    expect(connectivity$.error.peek()).toBe('old connection failure')
  })

  it('keeps errors belonging to a different account', async () => {
    setSyncError('other-account', 'other connection failure')
    await reconnect()
    expect(connectivity$.error.peek()).toBe('other connection failure')
    expect(connectivity$.account.peek()).toBe('other-account')
  })

  it('creates a separate account when the email changes and keeps the original banner', async () => {
    const hook = renderHook(() => useAccountDialog())
    await act(async () =>
      hook.result.current.setForm((form) => ({ ...form, email: 'other@example.com', password: 'new-password' })),
    )
    await act(async () => {
      await hook.result.current.save()
    })
    expect(saved).not.toHaveProperty('id')
    expect(connectivity$.account.peek()).toBe(account.id)
    expect(connectivity$.error.peek()).toBe('old connection failure')
  })

  it('omits the password only when editing the same mailbox', async () => {
    accounts$.set([{ ...account, needs_reconnect: false }])
    const hook = renderHook(() => useAccountDialog())
    expect(hook.result.current.saveDisabled).toBe(false)
    await act(async () => {
      await hook.result.current.save()
    })
    expect(saved.id).toBe(account.id)
    expect(saved).not.toHaveProperty('password')
  })

  it('requires a new password when an edit changes the mailbox', async () => {
    accounts$.set([{ ...account, needs_reconnect: false }])
    const hook = renderHook(() => useAccountDialog())
    await act(async () => hook.result.current.setForm((form) => ({ ...form, email: 'other@example.com' })))
    expect(hook.result.current.saveDisabled).toBe(true)
  })

  for (const provider of ['gmail', 'outlook'] as const) {
    for (const sameMailbox of [true, false]) {
      it(`${provider} polling ${sameMailbox ? 'reuses the ID and clears the banner for the same mailbox' : 'keeps the original account and banner for a different sign-in'}`, async () => {
        jest.useFakeTimers()
        ui$.setupMode.set(provider)
        profileEmail = sameMailbox ? ` ${account.email.toUpperCase()} ` : 'other@example.com'
        const hook = renderHook(() => useAccountDialog())
        await act(async () => {
          await hook.result.current.beginOAuth(provider)
        })
        await act(async () => {
          jest.advanceTimersByTime(1000)
        })
        expect(saved).not.toBeNull()
        expect(ui$.googleCalendarSetupEmail.peek()).toBe('')
        if (sameMailbox) {
          expect(saved.id).toBe(account.id)
          expect(connectivity$.error.peek()).toBeNull()
        } else {
          expect(saved).not.toHaveProperty('id')
          expect(connectivity$.error.peek()).toBe('old connection failure')
        }
      })
    }
  }

  function prepareGmailSetup() {
    jest.useFakeTimers()
    accounts$.set([])
    ui$.setupMode.set('gmail')
    ui$.reconnectAccountId.set('')
    ui$.setupOpen.set(false)
    ui$.system.set({ gmail_oauth_configured: true } as SystemCheck)
    profileEmail = 'new-mailbox@example.test'
    listedAccounts = [{ ...account, id: profileEmail, email: profileEmail, provider: 'gmail', needs_reconnect: false }]
  }

  // App swaps the first-run form for the normal window once boot has loaded the mailbox.
  function SetupFlow() {
    const accounts = useValue(accounts$)
    const setupOpen = useValue(ui$.setupOpen)
    if (!accounts.length) return <AccountDialog variant="setup" />
    return (
      <>
        {setupOpen && <AccountDialog />}
        <GoogleCalendarSetupDialog />
      </>
    )
  }

  async function signIn() {
    render(<SetupFlow />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Sign in with Google' }))
    })
    await act(async () => {
      jest.advanceTimersByTime(1000)
    })
    expect(accounts$.peek().some((entry) => entry.email === profileEmail)).toBe(true)
  }

  it('offers Google Calendar after the first Gmail mailbox is saved and lets the user skip', async () => {
    prepareGmailSetup()
    await signIn()
    expect(screen.getByRole('dialog', { name: 'Connect Google Calendar' })).toBeTruthy()
    expect(screen.getByText(profileEmail)).toBeTruthy()
    expect(calendarRequests).toEqual([])

    fireEvent.click(screen.getByRole('button', { name: 'Skip' }))
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(accounts$.peek()[0]?.email).toBe(profileEmail)
    expect(calendarRequests).toEqual([])
  })

  it('opens separate consent for the new Gmail address and keeps mail available after a decline', async () => {
    prepareGmailSetup()
    accounts$.set([account])
    listedAccounts.unshift(account)
    ui$.setupOpen.set(true)
    rejectCalendar = true
    await signIn()

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Connect Google Calendar' }))
    })
    expect(calendarRequests).toEqual([{ login_hint: profileEmail, account_id: undefined }])
    expect(screen.getByRole('alert').textContent).toBe('Sign-in was declined')
    expect(accounts$.peek().map((entry) => entry.email)).toEqual([account.email, profileEmail])

    rejectCalendar = false
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Connect Google Calendar' }))
    })
    expect(calendarRequests).toHaveLength(2)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('keeps Calendar setup out of the flow when Calendar is disabled', async () => {
    prepareGmailSetup()
    settings$.calendarEnabled.set(false)
    await signIn()
    expect(screen.queryByRole('dialog')).toBeNull()
  })
})
