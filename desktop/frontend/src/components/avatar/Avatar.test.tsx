import { afterEach, beforeEach, expect, it } from 'bun:test'
import { act, cleanup, render, waitFor } from '@testing-library/react'
import { settings$ } from '../../states/settings'
import { Avatar } from './Avatar'

let sequence = 0
let email: string
let calls: Array<{ command: string; payload: any }>
let resolveImage: (image: { src: string; kind: string }) => void

beforeEach(() => {
  email = `sender${sequence++}@example.com`
  calls = []
  settings$.showRealAvatars.set(true)
  ;(window as any).go = {
    main: {
      App: {
        Invoke: (command: string, payload: any) => {
          if (command !== 'avatar.resolve') return Promise.resolve({})
          calls.push({ command, payload })
          return new Promise((resolve) => {
            resolveImage = resolve
          })
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

it('uses the shared sender resolver and displays its favicon', async () => {
  const view = render(<Avatar name="Cloudflare" email={email} size={40} />)
  await waitFor(() => expect(calls).toHaveLength(1))
  expect(calls[0]).toEqual({ command: 'avatar.resolve', payload: { email, size: 80 } })
  await act(async () => resolveImage({ src: 'data:image/png;base64,aGVsbG8=', kind: 'favicon' }))
  expect(view.getByRole('img').getAttribute('src')).toBe('data:image/png;base64,aGVsbG8=')
})

it('ignores an in-flight result after sender images are disabled', async () => {
  const view = render(<Avatar name="Cloudflare" email={email} />)
  await waitFor(() => expect(calls).toHaveLength(1))
  await act(async () => settings$.showRealAvatars.set(false))
  await act(async () => resolveImage({ src: 'data:image/png;base64,aGVsbG8=', kind: 'favicon' }))
  expect(view.queryByRole('img')).toBeNull()
  expect(view.getByText('C')).toBeTruthy()
})

it('does not resolve sender images when disabled or a manual avatar is provided', async () => {
  settings$.showRealAvatars.set(false)
  const view = render(<Avatar name="Cloudflare" email={email} />)
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 30))
  })
  expect(calls).toHaveLength(0)
  view.rerender(<Avatar name="Cloudflare" email={email} src="/media/manual.png" />)
  await act(async () => settings$.showRealAvatars.set(true))
  expect(view.getByRole('img').getAttribute('src')).toBe('/media/manual.png')
  expect(calls).toHaveLength(0)
})

it('renders a cached image immediately on remount without another IPC call', async () => {
  const first = render(<Avatar name="Cloudflare" email={email} />)
  await waitFor(() => expect(calls).toHaveLength(1))
  await act(async () => resolveImage({ src: 'data:image/png;base64,aGVsbG8=', kind: 'favicon' }))
  first.unmount()
  const second = render(<Avatar name="Cloudflare" email={email} />)
  expect(second.getByRole('img').getAttribute('src')).toBe('data:image/png;base64,aGVsbG8=')
  expect(second.queryByText('C')).toBeNull()
  expect(calls).toHaveLength(1)
})

it('retries an empty result on remount instead of caching initials', async () => {
  const first = render(<Avatar name="Cloudflare" email={email} />)
  await waitFor(() => expect(calls).toHaveLength(1))
  await act(async () => resolveImage({ src: '', kind: 'none' }))
  first.unmount()
  const second = render(<Avatar name="Cloudflare" email={email} />)
  await waitFor(() => expect(calls).toHaveLength(2))
  await act(async () => resolveImage({ src: 'data:image/png;base64,aGVsbG8=', kind: 'favicon' }))
  expect(second.getByRole('img')).toBeTruthy()
})
