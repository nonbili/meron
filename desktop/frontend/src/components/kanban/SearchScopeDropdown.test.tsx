import { afterEach, beforeEach, expect, it } from 'bun:test'
import { cleanup, render } from '@testing-library/react'
import { accounts$ } from '../../states/accounts'
import { kanbanColumnKey } from '../../states/kanban'
import { settings$ } from '../../states/settings'
import type { Account } from '../../types'
import { SearchScopeDropdown } from './SearchScopeDropdown'

const account: Account = {
  id: 'acc1',
  email: 'nils@example.com',
  display_name: 'Nils Example',
  provider: 'custom',
  auth_type: 'password',
  imap_host: 'imap.example.com',
  imap_port: 993,
  smtp_host: 'smtp.example.com',
  smtp_port: 465,
  tls: true,
}
const column = { accountId: 'acc1', folderId: 'INBOX' }

let resolveCalls: unknown[]
let idleCallback: unknown

beforeEach(() => {
  resolveCalls = []
  settings$.showRealAvatars.set(true)
  // Avatars resolve on idle; running that at once lets a test see that no
  // lookup was made without waiting one out.
  idleCallback = (window as any).requestIdleCallback
  ;(window as any).requestIdleCallback = (callback: () => void) => {
    callback()
    return 0
  }
  ;(window as any).go = {
    main: {
      App: {
        Invoke: (command: string, payload: unknown) => {
          if (command === 'avatar.resolve') resolveCalls.push(payload)
          return Promise.resolve({ src: 'data:image/png;base64,aGVsbG8=', kind: 'gravatar' })
        },
      },
    },
  }
})

afterEach(() => {
  cleanup()
  accounts$.set([])
  settings$.showRealAvatars.set(false)
  ;(window as any).requestIdleCallback = idleCallback
  delete (window as any).go
})

function renderScoped() {
  return render(
    <SearchScopeDropdown value={kanbanColumnKey(column)} onChange={() => undefined} visibleColumns={[column]} />,
  )
}

// The account's own picture is set (or removed) in its settings. A Gravatar for
// its address is a sender image, so a removed picture must leave the initials.
it("shows an account's initials, not its Gravatar, when it has no picture", () => {
  accounts$.set([account])
  const view = renderScoped()
  expect(resolveCalls).toHaveLength(0)
  expect(view.queryByRole('img')).toBeNull()
  expect(view.getByText('NE')).toBeTruthy()
})

it("shows the account's own picture when one is set", () => {
  accounts$.set([{ ...account, avatar_url: '/media/me.png' }])
  expect(renderScoped().getByRole('img').getAttribute('src')).toBe('/media/me.png')
})
