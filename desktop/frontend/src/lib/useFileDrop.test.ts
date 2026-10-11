import { afterEach, describe, expect, it, jest } from 'bun:test'
import { act, renderHook } from '@testing-library/react'
import type { DragEvent } from 'react'
import {
  deliverNativeDrop,
  dragMayHoldImages,
  isExternalFileDrop,
  routeComposerDrop,
  resetFileDropStateForTests,
  useFileDrop,
} from './useFileDrop'

const file = (name: string, type: string) => new File(['x'], name, { type })

// Just enough of a React drag event for the handlers: they read the
// DataTransfer and call preventDefault.
function dragEvent(types: string[], files: File[] = [], defaultPrevented = false) {
  return {
    dataTransfer: { types, files, dropEffect: 'none' },
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
  // address; the file itself arrives from Go after the drop (filedrop.go).
  afterEach(() => resetFileDropStateForTests())

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

  it('hands over the files Go reads after the drop', () => {
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles))
    const drop = dragEvent(['text/uri-list'])
    act(() => result.current.dropHandlers.onDrop(drop))
    expect(drop.preventDefault).toHaveBeenCalled()
    expect(onFiles).not.toHaveBeenCalled()

    const files = [file('photo.png', 'image/png')]
    deliverNativeDrop(files)
    // Not handled by a child: the composer body can't place a file it never got.
    expect(onFiles).toHaveBeenCalledWith(files, expect.objectContaining({ handledByChild: false, zone: null }))
  })

  it('also takes files that arrive from Go before the drop event', () => {
    const onFiles = jest.fn()
    const { result } = renderHook(() => useFileDrop(onFiles))
    const files = [file('a.pdf', 'application/pdf')]
    deliverNativeDrop(files)
    act(() => result.current.dropHandlers.onDrop(dragEvent(['text/uri-list'], [], true)))
    expect(onFiles).toHaveBeenCalledWith(files, expect.objectContaining({ handledByChild: false }))
  })

  it('names the drop area at the drop, not when the files arrive', () => {
    // The overlay the drop landed on is gone by the time Go's files arrive, so
    // asking where it was then would always answer "nowhere".
    const onFiles = jest.fn()
    const surface = document.createElement('div')
    document.body.append(surface)
    const zoneOf = (target: EventTarget | null) => (target === surface && surface.isConnected ? 'body' : 'elsewhere')
    const { result } = renderHook(() => useFileDrop(onFiles, { zoneOf }))
    const drop = Object.assign(dragEvent(['text/uri-list']), { target: surface })
    act(() => result.current.dropHandlers.onDrop(drop))
    surface.remove()

    const files = [file('photo.png', 'image/png')]
    deliverNativeDrop(files)
    expect(onFiles).toHaveBeenCalledWith(files, { handledByChild: false, zone: 'body' })
  })

  it('ignores files from Go when no drop on this page asked for them', () => {
    const onFiles = jest.fn()
    renderHook(() => useFileDrop(onFiles))
    deliverNativeDrop([file('a.pdf', 'application/pdf')])
    expect(onFiles).not.toHaveBeenCalled()
  })

  it('ignores a drag that started inside the page, such as a link in a message', () => {
    const { result } = renderHook(() => useFileDrop(() => {}))
    act(() => {
      window.dispatchEvent(new Event('dragstart'))
      result.current.dropHandlers.onDragEnter(dragEvent(['text/uri-list']))
    })
    expect(result.current.dragging).toBe(false)
  })
})

describe('isExternalFileDrop', () => {
  const transfer = (types: string[], files: File[] = []) => ({ types, files }) as unknown as DataTransfer
  afterEach(() => resetFileDropStateForTests())

  it('recognises a Linux file manager drop: file addresses, no files', () => {
    expect(isExternalFileDrop(transfer(['text/uri-list', 'text/html']))).toBe(true)
  })

  it('leaves drops with real files, plain text, and drags from inside the page alone', () => {
    expect(isExternalFileDrop(transfer(['Files'], [file('a.png', 'image/png')]))).toBe(false)
    expect(isExternalFileDrop(transfer(['text/plain']))).toBe(false)
    window.dispatchEvent(new Event('dragstart'))
    expect(isExternalFileDrop(transfer(['text/uri-list']))).toBe(false)
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
