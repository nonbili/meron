import { afterEach, expect, it } from 'bun:test'
import { cleanup, render } from '@testing-library/react'
import type { Account, Message } from '../../types'
import { settings$ } from '../../states/settings'
import { ThreadListItem } from './ThreadListItem'

const thread = {
  id: 'm1',
  account_id: 'acc',
  folder_id: 'acc#INBOX',
  thread_id: 'acc#INBOX#t1',
  from_name: 'Alice Doe',
  from_addr: 'alice@example.com',
  subject: 'Hello',
  preview: 'Hi there',
  to: '',
  body: '',
  unread: false,
  starred: false,
  has_attachments: false,
  date: 1767225600,
} as Message

const account = { id: 'acc', email: 'studio@example.com', display_name: 'Studio' } as Account

// Avatars are sized inline, so their diameters tell the sender's from the account's.
const avatarSizes = (container: HTMLElement) =>
  Array.from(container.querySelectorAll<HTMLElement>('[style*="width"]')).map((el) => el.style.width)

afterEach(() => {
  cleanup()
  settings$.showThreadAvatars.set(true)
  settings$.threadPreviewLine.set(false)
})

it('badges the account on the sender avatar when avatars are shown', () => {
  const { container } = render(<ThreadListItem thread={thread} badgeAccount={account} onSelect={() => {}} />)
  expect(avatarSizes(container)).toEqual(['40px', '16px'])
})

it('drops the sender avatar but keeps an inline account mark when avatars are hidden', () => {
  settings$.showThreadAvatars.set(false)
  const { container } = render(<ThreadListItem thread={thread} badgeAccount={account} onSelect={() => {}} />)
  expect(avatarSizes(container)).toEqual(['14px'])
})

it('shows no avatar at all in single-account lists when avatars are hidden', () => {
  settings$.showThreadAvatars.set(false)
  const { container } = render(<ThreadListItem thread={thread} onSelect={() => {}} />)
  expect(avatarSizes(container)).toEqual([])
  expect(container.textContent).toContain('Alice Doe')
})

const colored = { ...account, color: '#3b82f6' }
const colorBar = (container: HTMLElement) => container.querySelector<HTMLElement>('[data-account-color]')

it('marks a coloured account with a bar instead of the avatar badge', () => {
  const { container } = render(<ThreadListItem thread={thread} badgeAccount={colored} onSelect={() => {}} />)
  expect(colorBar(container)?.style.backgroundColor).toBe('#3b82f6')
  expect(avatarSizes(container)).toEqual(['40px'])
})

it('keeps the colour bar, and no inline account avatar, when avatars are hidden', () => {
  settings$.showThreadAvatars.set(false)
  const { container } = render(<ThreadListItem thread={thread} badgeAccount={colored} onSelect={() => {}} />)
  expect(colorBar(container)).not.toBeNull()
  expect(avatarSizes(container)).toEqual([])
})

it('draws no colour bar outside mixed-account lists', () => {
  const { container } = render(<ThreadListItem thread={thread} onSelect={() => {}} />)
  expect(colorBar(container)).toBeNull()
})

// The subject's paragraph, and whether the preview follows it there or on its own line.
const lines = (container: HTMLElement) => Array.from(container.querySelectorAll('p')).map((p) => p.textContent)

it('runs the preview after the subject by default', () => {
  const { container } = render(<ThreadListItem thread={thread} onSelect={() => {}} />)
  expect(lines(container)).toEqual(['Hello - Hi there'])
})

it('gives the preview its own line under the subject when asked', () => {
  settings$.threadPreviewLine.set(true)
  const { container } = render(<ThreadListItem thread={thread} onSelect={() => {}} />)
  expect(lines(container)).toEqual(['Hello', 'Hi there'])
})
