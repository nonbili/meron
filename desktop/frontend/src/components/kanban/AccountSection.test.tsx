import { afterEach, beforeEach, expect, it } from 'bun:test'
import { act, cleanup, render } from '@testing-library/react'
import { settings$ } from '../../states/settings'
import { AccountSection } from './AccountSection'

let resolveCalls: unknown[]

beforeEach(() => {
  resolveCalls = []
  settings$.showRealAvatars.set(true)
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
  settings$.showRealAvatars.set(false)
  delete (window as any).go
})

const group = {
  accountId: 'acc1',
  label: 'Nils Example',
  avatarUrl: '',
  isRSS: false,
  folders: [],
  tree: [],
}

// The account's own picture is set (or removed) in its settings. A Gravatar for
// its address is a sender image, so a removed picture must leave the initials.
it("shows an account's initials, not its Gravatar, when it has no picture", async () => {
  const view = render(<AccountSection group={group} selected={new Set()} onToggle={() => undefined} />)
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 30))
  })
  expect(resolveCalls).toHaveLength(0)
  expect(view.queryByRole('img')).toBeNull()
  expect(view.getByText('NE')).toBeTruthy()
})

it("shows the account's own picture when one is set", () => {
  const view = render(
    <AccountSection group={{ ...group, avatarUrl: '/media/me.png' }} selected={new Set()} onToggle={() => undefined} />,
  )
  expect(view.getByRole('img').getAttribute('src')).toBe('/media/me.png')
})
