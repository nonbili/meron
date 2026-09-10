import { expect, test } from 'bun:test'
import { configurationRefresh } from './configurationRefresh'
import { settings$ } from '../states/settings'
import { accounts$ } from '../states/accounts'
import type { Account } from '../types'

test('configuration refresh only hydrates changed keys and ignores unrelated stale prefs', async () => {
  const calls: any[] = []
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string, payload: any) => {
          calls.push({ command, payload })
          if (command === 'account.list') return { accounts: [] }
          return { prefs: { signature: 'new signature', thread_list_width: 100, custom_themes: [] } }
        },
      },
    },
  }
  settings$.threadListWidth.set(420)
  settings$.language.set('ja')
  const refresh = configurationRefresh()
  await refresh.refresh({ keys: ['signature'] })
  expect(settings$.threadListWidth.get()).toBe(420)
  expect(settings$.signature.get()).toBe('new signature')
  expect(settings$.language.get()).toBe('ja')
  expect(calls.some((call) => call.command === 'account.list')).toBe(false)
  expect(calls.find((call) => call.command === 'app.prefsGet')?.payload).toEqual({ keys: ['signature'] })
  calls.length = 0
  await refresh.refresh()
  expect(settings$.language.get()).toBe('ja')
  expect(calls.some((call) => call.command === 'app.prefsGet')).toBe(false)
  refresh.dispose()
})

test('missing prefs are harmless', async () => {
  ;(window as any).go = {
    main: { App: { Invoke: async (command: string) => (command === 'account.list' ? { accounts: [] } : {}) } },
  }
  const refresh = configurationRefresh()
  await refresh.refresh({ keys: ['signature'] })
  refresh.dispose()
})

test('an engine restart cannot empty existing accounts and settings refresh independently', async () => {
  const account = { id: 'existing', email: 'existing@example.com' } as Account
  accounts$.set([account])
  ;(window as any).go = {
    main: {
      App: {
        Invoke: async (command: string) => {
          if (command === 'account.list') return { accounts: [] }
          return { prefs: { spell_check: false } }
        },
      },
    },
  }
  const refresh = configurationRefresh()
  await refresh.refresh()
  expect(accounts$.get()).toEqual([account])
  ;(window as any).go.main.App.Invoke = async (command: string) => {
    if (command === 'account.list') throw new Error('engine unavailable')
    return { prefs: { spell_check: false } }
  }
  settings$.spellCheck.set(true)
  await refresh.refresh({ keys: ['spell_check'] })
  expect(settings$.spellCheck.get()).toBe(false)
  expect(accounts$.get()).toEqual([account])
  refresh.dispose()
})
