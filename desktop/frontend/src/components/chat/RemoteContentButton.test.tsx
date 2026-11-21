import { afterEach, expect, it, mock } from 'bun:test'
import { cleanup, fireEvent, render } from '@testing-library/react'
import { settings$ } from '../../states/settings'
import { RemoteContentButton } from './RemoteContentButton'

afterEach(() => {
  cleanup()
  settings$.remoteImageSenders.set([])
})

it('adds the sender of a plain message and marks the sender as allowed', () => {
  const view = render(<RemoteContentButton blocked={false} hiddenRemoteCount={0} senderAddress="News@Example.com" />)
  fireEvent.click(view.getByRole('button', { name: 'Always load remote content' }))
  fireEvent.click(view.getByRole('button', { name: /always allow from.*news@example.com/i }))

  expect(settings$.remoteImageSenders.peek()).toEqual(['news@example.com'])
  expect(view.getByRole('button', { name: 'Sender already allowed' }).hasAttribute('disabled')).toBe(true)
})

it('offers a one-time reveal even when blocked media was not detected', () => {
  const reveal = mock()
  const view = render(
    <RemoteContentButton blocked={false} canReveal hiddenRemoteCount={0} senderAddress="" onReveal={reveal} />,
  )
  fireEvent.click(view.getByRole('button', { name: 'Always load remote content' }))
  fireEvent.click(view.getByRole('button', { name: 'Show remote content' }))

  expect(reveal).toHaveBeenCalledTimes(1)
  expect(settings$.remoteImageSenders.peek()).toEqual([])
})
