import { afterEach, describe, expect, it } from 'bun:test'
import {
  EMPTY_PROXY,
  hydrateSettings,
  isProxyUsable,
  normalizeSenderAddr,
  sanitizeKanbanBoards,
  sanitizeProxy,
  setRemoteImageSender,
  settings$,
  WRITE_SESSION,
} from './settings'

const baseBoard = {
  id: 'kb-1',
  name: 'Work',
  columns: [{ accountId: 'acc-1', folderId: 'inbox' }],
}

describe('sanitizeKanbanBoards', () => {
  it('keeps boards without customization fields untouched', () => {
    expect(sanitizeKanbanBoards([baseBoard])).toEqual([baseBoard])
  })

  it('accepts an app-managed avatar url', () => {
    const boards = sanitizeKanbanBoards([{ ...baseBoard, avatarUrl: '/media/avatars/kb-1/a.png' }])
    expect(boards?.[0].avatarUrl).toBe('/media/avatars/kb-1/a.png')
  })

  it('drops avatar urls outside /media/avatars/', () => {
    for (const avatarUrl of ['https://example.com/a.png', '/media/wallpapers/kb-1/a.png', 7, '']) {
      const boards = sanitizeKanbanBoards([{ ...baseBoard, avatarUrl }])
      expect(boards?.[0].avatarUrl).toBeUndefined()
    }
  })

  it('accepts preset and custom wallpapers', () => {
    expect(
      sanitizeKanbanBoards([{ ...baseBoard, wallpaper: { kind: 'preset', presetId: 'dots' } }])?.[0].wallpaper,
    ).toEqual({ kind: 'preset', presetId: 'dots' })
    expect(
      sanitizeKanbanBoards([{ ...baseBoard, wallpaper: { kind: 'custom', url: '/media/wallpapers/kb-1/w.png' } }])?.[0]
        .wallpaper,
    ).toEqual({ kind: 'custom', url: '/media/wallpapers/kb-1/w.png' })
  })

  it('drops invalid wallpapers', () => {
    for (const wallpaper of [
      { kind: 'preset', presetId: 'nope' },
      { kind: 'custom', url: 'https://example.com/w.png' },
      'dots',
      null,
    ]) {
      const boards = sanitizeKanbanBoards([{ ...baseBoard, wallpaper }])
      expect(boards?.[0].wallpaper).toBeUndefined()
    }
  })
})

describe('spellCheck setting', () => {
  afterEach(() => {
    settings$.spellCheck.set(true)
  })

  it('defaults spell check on', () => {
    expect(settings$.spellCheck.get()).toBe(true)
  })

  it('hydrates a persisted spell check preference', () => {
    hydrateSettings({ spell_check: false })
    expect(settings$.spellCheck.get()).toBe(false)

    hydrateSettings({ spell_check: true })
    expect(settings$.spellCheck.get()).toBe(true)
  })

  it('ignores invalid persisted spell check values', () => {
    settings$.spellCheck.set(false)
    hydrateSettings({ spell_check: 'true' })
    expect(settings$.spellCheck.get()).toBe(false)
  })
})

describe('kanban scroll lock setting', () => {
  afterEach(() => {
    settings$.kanbanLockScroll.set(false)
  })

  it('leaves the board free to auto-scroll by default', () => {
    expect(settings$.kanbanLockScroll.get()).toBe(false)
  })

  it('hydrates a persisted lock', () => {
    hydrateSettings({ kanban_lock_scroll: true })
    expect(settings$.kanbanLockScroll.get()).toBe(true)
  })

  it('ignores invalid persisted lock values', () => {
    hydrateSettings({ kanban_lock_scroll: 'true' })
    expect(settings$.kanbanLockScroll.get()).toBe(false)
  })
})

describe('proxy setting', () => {
  afterEach(() => {
    settings$.proxy.set(EMPTY_PROXY)
  })

  it('defaults to no proxy', () => {
    expect(settings$.proxy.get()).toEqual(EMPTY_PROXY)
  })

  it('hydrates a persisted proxy', () => {
    hydrateSettings({ proxy: { mode: 'socks5', host: ' 127.0.0.1 ', port: 1080, username: 'u', password: 'p' } })
    expect(settings$.proxy.get()).toEqual({
      mode: 'socks5',
      host: '127.0.0.1',
      port: 1080,
      username: 'u',
      password: 'p',
    })
  })

  it('rejects unknown modes and out-of-range ports', () => {
    expect(sanitizeProxy({ mode: 'ftp', host: 'h', port: 1 })).toBeNull()
    expect(sanitizeProxy('socks5')).toBeNull()
    expect(sanitizeProxy({ mode: 'http', host: 'h', port: 70000 })?.port).toBe(0)
    expect(sanitizeProxy({ mode: 'http', host: 'h', port: -1 })?.port).toBe(0)
  })

  it('treats a half-filled proxy as unusable', () => {
    expect(isProxyUsable({ mode: 'off', host: 'h', port: 1080, username: '', password: '' })).toBe(false)
    expect(isProxyUsable({ mode: 'http', host: '', port: 8080, username: '', password: '' })).toBe(false)
    expect(isProxyUsable({ mode: 'http', host: 'h', port: 0, username: '', password: '' })).toBe(false)
    expect(isProxyUsable({ mode: 'http', host: 'h', port: 8080, username: '', password: '' })).toBe(true)
  })
})

describe('remote content allowlist', () => {
  afterEach(() => {
    settings$.remoteImageSenders.set([])
  })

  it('stores the bare lowercased address', () => {
    expect(normalizeSenderAddr(' News <News@Example.COM> ')).toBe('news@example.com')
    expect(normalizeSenderAddr('  ')).toBe('')
  })

  it('allows a sender once, and takes the allowance back', () => {
    setRemoteImageSender('News <News@Example.com>', true)
    setRemoteImageSender('news@example.com', true)
    expect(settings$.remoteImageSenders.get()).toEqual(['news@example.com'])

    setRemoteImageSender('NEWS@example.com', false)
    expect(settings$.remoteImageSenders.get()).toEqual([])
  })

  it('ignores an address that normalizes to nothing', () => {
    setRemoteImageSender('  ', true)
    expect(settings$.remoteImageSenders.get()).toEqual([])
  })

  it('survives a malformed persisted value', () => {
    settings$.remoteImageSenders.set(['news@example.com'])
    for (const remote_image_senders of [{}, true, 'news@example.com', 7, null]) {
      expect(() => hydrateSettings({ remote_image_senders })).not.toThrow()
      // A row we cannot read leaves the in-memory list as it was.
      expect(settings$.remoteImageSenders.get()).toEqual(['news@example.com'])
    }
  })

  it('normalizes persisted senders before deduping them', () => {
    hydrateSettings({
      remote_image_senders: ['News <News@Example.com>', 'news@example.com', '', 7, 'other@example.test'],
    })
    expect(settings$.remoteImageSenders.get()).toEqual(['news@example.com', 'other@example.test'])
  })
})

describe('settings persistence', () => {
  it('stamps each write so a straggler cannot overwrite the newest value', async () => {
    const wails = (window as any).go
    const calls: any[] = []
    ;(window as any).go = {
      main: {
        App: {
          Invoke: (_command: string, payload: any) => {
            calls.push(payload)
            // The first write hangs: a later one must not wait behind it.
            return calls.length === 1 ? new Promise<void>(() => {}) : Promise.resolve()
          },
        },
      },
    }

    try {
      settings$.remoteImageSenders.set(['news@example.com'])
      settings$.remoteImageSenders.set([])
      await new Promise((resolve) => setTimeout(resolve, 0))

      expect(calls.map((call) => call.value)).toEqual([['news@example.com'], []])
      // Rising per key, so the sidecar can drop whichever lands late.
      expect(calls[1].seq).toBe(calls[0].seq + 1)
      expect(calls.every((call) => call.key === 'remote_image_senders')).toBe(true)
      // Every write carries the label boot hands to the sidecar, so a reload's
      // restarted counters are not compared against the stamps recorded before
      // it — and its stragglers cannot overwrite what this session writes.
      expect(WRITE_SESSION).toBeTruthy()
      expect(calls.map((call) => call.session)).toEqual([WRITE_SESSION, WRITE_SESSION])
    } finally {
      ;(window as any).go = wails
      settings$.remoteImageSenders.set([])
    }
  })
})
