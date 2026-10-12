import { afterEach, beforeEach, describe, expect, it, jest } from 'bun:test'
import { act, renderHook } from '@testing-library/react'
import type { DragEvent } from 'react'
import {
  dragMayHoldImages,
  isExternalFileDrop,
  routeComposerDrop,
  resetFileDropStateForTests,
  trackInternalDrags,
  useFileDrop,
} from './useFileDrop'

const file = (name: string, type: string) => new File(['x'], name, { type })

// Just enough of a React drag event for the handlers: they read the
// DataTransfer and call preventDefault. `uriList` is what a drop lets the page
// read as text/uri-list.
function dragEvent(types: string[], files: File[] = [], defaultPrevented = false, uriList = '') {
  return {
    dataTransfer: {
      types,
      files,
      dropEffect: 'none',
      getData: (type: string) => (type === 'text/uri-list' ? uriList : ''),
    },
    defaultPrevented,
    preventDefault: jest.fn(),
  } as unknown as DragEvent<HTMLElement> & { preventDefault: ReturnType<typeof jest.fn> }
}

describe('useFileDrop', () => {
  afterEach(() => resetFileDropStateForTests())

  it('shows the drop state only for drags that carry files', () => {
    const { result } = renderHook(() => useFileDrop(() => {}))
    act(() => result.current.dropHandlers.onDragEnter(dragEvent(['text/plain'])))
    expect(result.current.dragging).toBe(false)
    act(() => result.current.dropHandlers.onDragEnter(dragEvent(['Files'])))
    expect(result.current.dragging).toBe(true)
  })

  it('stays up while the drag moves between nested elements', () => {
    const { result } = renderHook(() => useFileDrop(() => {}))
    act(() => {
      result.current.dropHandlers.onDragEnter(dragEvent(['Files']))
      result.current.dropHandlers.onDragEnter(dragEvent(['Files']))
      result.current.dropHandlers.onDragLeave(dragEvent(['Files']))
    })
    expect(result.current.dragging).toBe(true)
    act(() => result.current.dropHandlers.onDragLeave(dragEvent(['Files'])))
    expect(result.current.dragging).toBe(false)
  })

  it('accepts the drag over it, so the webview does not open the file', () => {
    const { result } = renderHook(() => useFileDrop(() => {}))
    const over = dragEvent(['Files'])
    act(() => result.current.dropHandlers.onDragOver(over))
    expect(over.preventDefault).toHaveBeenCalled()
    const textOver = dragEvent(['text/plain'])
    act(() => result.current.dropHandlers.onDragOver(textOver))
    expect(textOver.preventDefault).not.toHaveBeenCalled()
  })

  it('hands the dropped files over and clears the drop state', () => {
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles))
    const files = [file('a.pdf', 'application/pdf')]
    const drop = dragEvent(['Files'], files)
    act(() => {
      result.current.dropHandlers.onDragEnter(dragEvent(['Files']))
      result.current.dropHandlers.onDrop(drop)
    })
    expect(drop.preventDefault).toHaveBeenCalled()
    expect(onFiles).toHaveBeenCalledWith(files, expect.objectContaining({ handledByChild: false }))
    expect(result.current.dragging).toBe(false)
  })

  it('tells whether a child already handled the drop', () => {
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles))
    const files = [file('photo.png', 'image/png')]
    act(() => result.current.dropHandlers.onDrop(dragEvent(['Files'], files, true)))
    expect(onFiles).toHaveBeenCalledWith(files, expect.objectContaining({ handledByChild: true }))
  })

  it('ignores drags entirely when disabled', () => {
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles, { enabled: false }))
    const drop = dragEvent(['Files'], [file('a.pdf', 'application/pdf')])
    act(() => {
      result.current.dropHandlers.onDragEnter(dragEvent(['Files']))
      result.current.dropHandlers.onDrop(drop)
    })
    expect(result.current.dragging).toBe(false)
    expect(onFiles).not.toHaveBeenCalled()
    expect(drop.preventDefault).not.toHaveBeenCalled()
  })
})

describe('useFileDrop on Linux (WebKitGTK)', () => {
  // A file dragged from the file manager is announced only as its file://
  // address; the page asks Go for the file itself after the drop (filedrop.go).
  const NOTE = 'file:///home/me/my%20note.txt'
  let requests: { command: string; payload: any }[] = []
  let answer: unknown = { files: [] }

  beforeEach(() => {
    resetFileDropStateForTests({ fileAddressDrops: true })
    requests = []
    answer = { files: [{ name: 'photo.png', mime: 'image/png', data: 'data:image/png;base64,eA==' }] }
    ;(window as any).go = {
      main: {
        App: {
          Invoke: async (command: string, payload: unknown) => {
            requests.push({ command, payload })
            return answer
          },
        },
      },
    }
  })
  afterEach(() => {
    resetFileDropStateForTests()
    delete (window as any).go
  })

  const settled = () => act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)))

  it('treats a file manager drag announced as text/uri-list as a file drag', () => {
    const { result } = renderHook(() => useFileDrop(() => {}))
    const over = dragEvent(['text/uri-list', 'text/html'])
    act(() => {
      result.current.dropHandlers.onDragEnter(dragEvent(['text/uri-list', 'text/html']))
      result.current.dropHandlers.onDragOver(over)
    })
    expect(result.current.dragging).toBe(true)
    expect(over.preventDefault).toHaveBeenCalled()
  })

  it('asks Go for the dropped files by their addresses and hands them over', async () => {
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles))
    const drop = dragEvent(['text/uri-list'], [], true, `${NOTE}\r\n`)
    act(() => result.current.dropHandlers.onDrop(drop))
    expect(drop.preventDefault).toHaveBeenCalled()
    expect(onFiles).not.toHaveBeenCalled()
    await settled()

    expect(requests).toEqual([{ command: 'system.readDroppedFiles', payload: { uris: [NOTE] } }])
    // Not handled by a child: the composer body can't place a file it never got.
    expect(onFiles).toHaveBeenCalledTimes(1)
    const [files, info] = onFiles.mock.calls[0]
    expect(files.map((f: File) => [f.name, f.type])).toEqual([['photo.png', 'image/png']])
    expect(info).toEqual({ handledByChild: false, zone: null })
  })

  it('leaves the choice to Go when the webview hides the addresses', async () => {
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles))
    act(() => result.current.dropHandlers.onDrop(dragEvent(['text/uri-list'])))
    await settled()
    expect(requests).toEqual([{ command: 'system.readDroppedFiles', payload: { uris: [] } }])
    expect(onFiles).toHaveBeenCalledTimes(1)
  })

  it('names the drop area at the drop, not when the files arrive', async () => {
    // The overlay the drop landed on is gone by the time Go's files arrive, so
    // asking where it was then would always answer "nowhere".
    const onFiles = jest.fn()
    const surface = document.createElement('div')
    document.body.append(surface)
    const zoneOf = (target: EventTarget | null) => (target === surface && surface.isConnected ? 'body' : 'elsewhere')
    const { result } = renderHook(() => useFileDrop(onFiles, { zoneOf }))
    const drop = Object.assign(dragEvent(['text/uri-list'], [], false, NOTE), { target: surface })
    act(() => result.current.dropHandlers.onDrop(drop))
    surface.remove()
    await settled()
    expect(onFiles).toHaveBeenCalledWith(expect.anything(), { handledByChild: false, zone: 'body' })
  })

  it('takes nothing when Go has no files for the drop', async () => {
    answer = { files: [] }
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles))
    act(() => result.current.dropHandlers.onDrop(dragEvent(['text/uri-list'], [], false, NOTE)))
    await settled()
    expect(onFiles).not.toHaveBeenCalled()
  })

  it('drops files that arrive after the reader moved to another conversation', async () => {
    const onFiles = jest.fn()
    const { result, rerender } = renderHook(({ scope }) => useFileDrop(onFiles, { scope }), {
      initialProps: { scope: 'thread-a' },
    })
    act(() => result.current.dropHandlers.onDrop(dragEvent(['text/uri-list'], [], false, NOTE)))
    rerender({ scope: 'thread-b' })
    await settled()
    expect(requests).toHaveLength(1)
    expect(onFiles).not.toHaveBeenCalled()
  })

  it('drops files that arrive after the drop area is gone', async () => {
    const onFiles = jest.fn()
    const { result, unmount } = renderHook(() => useFileDrop(onFiles))
    act(() => result.current.dropHandlers.onDrop(dragEvent(['text/uri-list'], [], false, NOTE)))
    unmount()
    await new Promise<void>((resolve) => setTimeout(resolve, 0))
    expect(onFiles).not.toHaveBeenCalled()
  })

  it('lets a text field take a link dropped from another app, and cancels it elsewhere', () => {
    const { result } = renderHook(() => useFileDrop(() => {}))
    const field = document.createElement('textarea')
    const onField = Object.assign(dragEvent(['text/uri-list'], [], false, 'https://example.com/'), { target: field })
    const elsewhere = Object.assign(dragEvent(['text/uri-list'], [], false, 'https://example.com/'), {
      target: document.createElement('div'),
    })
    act(() => {
      result.current.dropHandlers.onDrop(onField)
      result.current.dropHandlers.onDrop(elsewhere)
    })
    expect(onField.preventDefault).not.toHaveBeenCalled()
    expect(elsewhere.preventDefault).toHaveBeenCalled()
  })

  it('says when a drag may be a link, so the overlay leaves text fields reachable', () => {
    const { result } = renderHook(() => useFileDrop(() => {}))
    act(() => result.current.dropHandlers.onDragEnter(dragEvent(['text/uri-list'])))
    expect(result.current.mayBeLink).toBe(true)
    act(() => result.current.dropHandlers.onDragEnter(dragEvent(['Files'])))
    expect(result.current.mayBeLink).toBe(false)
  })

  it('does not ask Go about a link dropped from another app', async () => {
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles))
    act(() => result.current.dropHandlers.onDrop(dragEvent(['text/uri-list'], [], false, 'https://example.com/')))
    await settled()
    expect(requests).toEqual([])
    expect(onFiles).not.toHaveBeenCalled()
  })

  it('ignores a drag that started inside the page', () => {
    const { result } = renderHook(() => useFileDrop(() => {}))
    act(() => {
      window.dispatchEvent(new Event('dragstart'))
      result.current.dropHandlers.onDragEnter(dragEvent(['text/uri-list']))
    })
    expect(result.current.dragging).toBe(false)
  })

  it('ignores a drag that started in a message body, whose frame the page does not hear', () => {
    const frame = document.createElement('iframe')
    document.body.append(frame)
    const doc = frame.contentDocument!
    trackInternalDrags(doc)
    const { result } = renderHook(() => useFileDrop(() => {}))
    act(() => {
      doc.dispatchEvent(new Event('dragstart'))
      result.current.dropHandlers.onDragEnter(dragEvent(['text/uri-list']))
    })
    expect(result.current.dragging).toBe(false)
    frame.remove()
  })

  it('takes file drags again once the mouse moves, though the drag never reported its end', () => {
    const { result } = renderHook(() => useFileDrop(() => {}))
    act(() => {
      window.dispatchEvent(new Event('dragstart'))
      window.dispatchEvent(new Event('mousemove'))
      result.current.dropHandlers.onDragEnter(dragEvent(['text/uri-list']))
    })
    expect(result.current.dragging).toBe(true)
  })
})

describe('a link dragged in from another app (macOS, Windows)', () => {
  afterEach(() => resetFileDropStateForTests())

  it('is not a file drag: no overlay, and the drop is left alone', () => {
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles))
    const over = dragEvent(['text/uri-list', 'text/plain'])
    const drop = dragEvent(['text/uri-list', 'text/plain'], [], false, 'https://example.com/')
    act(() => {
      result.current.dropHandlers.onDragEnter(dragEvent(['text/uri-list', 'text/plain']))
      result.current.dropHandlers.onDragOver(over)
      result.current.dropHandlers.onDrop(drop)
    })
    expect(result.current.dragging).toBe(false)
    expect(over.preventDefault).not.toHaveBeenCalled()
    expect(drop.preventDefault).not.toHaveBeenCalled()
    expect(onFiles).not.toHaveBeenCalled()
  })
})

describe('isExternalFileDrop', () => {
  const transfer = (types: string[], files: File[] = [], uriList = '') =>
    ({ types, files, getData: (type: string) => (type === 'text/uri-list' ? uriList : '') }) as unknown as DataTransfer
  beforeEach(() => resetFileDropStateForTests({ fileAddressDrops: true }))
  afterEach(() => resetFileDropStateForTests())

  it('recognises a Linux file manager drop: file addresses, no files', () => {
    expect(isExternalFileDrop(transfer(['text/uri-list', 'text/html'], [], 'file:///home/me/a.png'))).toBe(true)
    expect(isExternalFileDrop(transfer(['text/uri-list']))).toBe(true)
  })

  it('leaves drops with real files, links, plain text, and drags from inside the page alone', () => {
    expect(isExternalFileDrop(transfer(['Files'], [file('a.png', 'image/png')]))).toBe(false)
    expect(isExternalFileDrop(transfer(['text/uri-list'], [], 'https://example.com/'))).toBe(false)
    expect(isExternalFileDrop(transfer(['text/plain']))).toBe(false)
    window.dispatchEvent(new Event('dragstart'))
    expect(isExternalFileDrop(transfer(['text/uri-list'], [], 'file:///home/me/a.png'))).toBe(false)
  })
})

describe('dragMayHoldImages', () => {
  const transfer = (items: { kind: string; type: string }[]) => ({ items }) as unknown as DataTransfer

  it('reads the file types where the webview announces them (Windows)', () => {
    expect(dragMayHoldImages(transfer([{ kind: 'file', type: 'image/png' }]))).toBe(true)
    expect(dragMayHoldImages(transfer([{ kind: 'file', type: 'application/pdf' }]))).toBe(false)
  })

  it('assumes images may come when the types are unknown (macOS, Linux)', () => {
    expect(dragMayHoldImages(transfer([{ kind: 'string', type: 'text/uri-list' }]))).toBe(true)
  })
})

describe('routeComposerDrop', () => {
  const image = file('photo.png', 'image/png')
  const pdf = file('doc.pdf', 'application/pdf')

  it('attaches everything dropped outside the message body', () => {
    expect(routeComposerDrop([image, pdf], { inBody: false, rich: true, handledByChild: false })).toEqual({
      inline: [],
      attach: [image, pdf],
    })
  })

  it('places images dropped on the body inline and attaches the rest', () => {
    expect(routeComposerDrop([image, pdf], { inBody: true, rich: true, handledByChild: false })).toEqual({
      inline: [image],
      attach: [pdf],
    })
  })

  it('leaves images to the editor when it already placed them (macOS, Windows)', () => {
    expect(routeComposerDrop([image, pdf], { inBody: true, rich: true, handledByChild: true })).toEqual({
      inline: [],
      attach: [pdf],
    })
  })

  it('attaches everything in plain-text mode, which has no inline images', () => {
    expect(routeComposerDrop([image, pdf], { inBody: true, rich: false, handledByChild: false })).toEqual({
      inline: [],
      attach: [image, pdf],
    })
  })
})
