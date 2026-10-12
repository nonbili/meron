import { useCallback, useEffect, useRef, useState } from 'react'
import type { DragEvent } from 'react'
import { invoke } from './bridge'
import { pickedToFile, type PickedFile } from './nativeFilePicker'

/** Which drop area the drop landed in (from `zoneOf`, decided at the drop), and
 *  whether a child (the composer body's editor) already took part of it. */
export type FileDropInfo = { handledByChild: boolean; zone: string | null }
type FilesHandler = (files: File[], drop: FileDropInfo) => void

// A drag that starts inside the page (a link or an image in a message) is never
// a file drop. Cards and columns use pointer events, not HTML drag and drop.
let internalDrag = false
const internalDragTargets = new WeakSet<Window | Document>()

/** Watches a document for drags that start in it. The page's own window is
 *  watched by useFileDrop; message bodies live in iframes, whose drag events
 *  never reach it, so HtmlFrame adds each frame's document. */
export function trackInternalDrags(target: Window | Document) {
  if (internalDragTargets.has(target)) return
  internalDragTargets.add(target)
  const ended = () => (internalDrag = false)
  target.addEventListener('dragstart', () => (internalDrag = true), true)
  target.addEventListener('dragend', ended, true)
  // Not capturing: the drop handlers below still need to see it as internal.
  target.addEventListener('drop', ended)
  // `dragend` is lost when the dragged element is removed mid-drag. The mouse
  // doesn't move the page during a drag, so once it does, the drag is over.
  target.addEventListener('mousemove', ended, { capture: true, passive: true })
}

// WebKitGTK on Linux announces a file from the file manager only as its file://
// address (`text/uri-list`). Elsewhere that type means a link dragged in from
// another app, which is not a file drop.
let fileAddressDrops = typeof navigator !== 'undefined' && /linux/i.test(navigator.userAgent)

/** Whether a drag carries files from outside the page (the file manager). macOS
 *  and Windows announce them as `Files`; Linux as their addresses, and the files
 *  themselves then come from Go (filedrop.go, readNativeDrop). A link dragged in
 *  on Linux looks the same until the drop (see droppedFileAddresses). */
function carriesFiles(data: DataTransfer | null | undefined): boolean {
  if (internalDrag || !data) return false
  const types = Array.from(data.types ?? [])
  return types.includes('Files') || (fileAddressDrops && types.includes('text/uri-list'))
}

/** The file:// addresses of a Linux file manager drop, readable only at the
 *  drop itself. Null when the drop holds addresses but none of a file: a link
 *  from another app. Empty when the webview doesn't tell. */
function droppedFileAddresses(data: DataTransfer): string[] | null {
  let list = ''
  try {
    list = data.getData?.('text/uri-list') ?? ''
  } catch {
    // Not readable: leave it to Go, which knows what was dropped.
  }
  const addresses = list
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith('#'))
  const files = addresses.filter((address) => /^file:/i.test(address))
  return addresses.length > 0 && files.length === 0 ? null : files
}

/** A Linux file manager drop: file addresses but no files, which arrive from Go
 *  instead. The composer body's editor must not paste the address as text. */
export function isExternalFileDrop(data: DataTransfer): boolean {
  if (!carriesFiles(data) || (data.files && data.files.length > 0)) return false
  return droppedFileAddresses(data) !== null
}

/** Whether a drag may hold images, for offering to insert them inline. Windows
 *  (WebView2) announces each file's type during the drag; WebKit on macOS and
 *  Linux lists no files until the drop (WebKit bug 223517), so there it may. */
export function dragMayHoldImages(data: DataTransfer | null | undefined): boolean {
  const fileTypes = Array.from(data?.items ?? [])
    .filter((item) => item.kind === 'file')
    .map((item) => item.type.toLowerCase())
  if (fileTypes.length === 0) return true
  return fileTypes.some((type) => type === '' || type.startsWith('image/'))
}

/** The files of a Linux file manager drop, read by Go. Go got the same paths
 *  from GTK and reads only those, and only now that the drop landed on a drop
 *  area (filedrop.go). */
async function readNativeDrop(addresses: string[]): Promise<File[]> {
  try {
    const result = await invoke<{ files?: PickedFile[] }>('system.readDroppedFiles', { uris: addresses })
    return (result?.files ?? []).map(pickedToFile)
  } catch (error) {
    console.error('Failed to read dropped files:', error)
    return []
  }
}

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false
  return !!target.closest('input, textarea, [contenteditable]:not([contenteditable="false"])')
}

export function resetFileDropStateForTests(options: { fileAddressDrops?: boolean } = {}) {
  internalDrag = false
  fileAddressDrops = options.fileAddressDrops ?? false
}

/**
 * Lets an element take files dropped onto it. `dragging` is true while files are
 * held over the element, for a drop overlay, and `mayHoldImages` says whether
 * they might be images; `onFiles` receives what was dropped and where: `zoneOf`
 * names the drop area from the drop's target, at the drop itself, since on Linux
 * the files arrive from Go only after the overlay they were dropped on is gone.
 * `scope` names what the files are for (the open conversation): if it changed
 * by the time they arrive, or the element is gone, they are dropped rather than
 * attached to something the reader didn't drop them on.
 *
 * `mayBeLink` is true for a Linux drag that announces only addresses, which a
 * link from another app does too: the overlay then must not cover a text field,
 * so that the link can still be dropped into it.
 *
 * Every other drag passes through untouched. Child elements fire their own
 * enter/leave pairs as the pointer moves across them, so a depth count keeps the
 * overlay from flickering.
 */
export function useFileDrop(
  onFiles: FilesHandler,
  {
    enabled = true,
    zoneOf,
    scope,
  }: { enabled?: boolean; zoneOf?: (target: EventTarget | null) => string | null; scope?: unknown } = {},
) {
  const [dragging, setDragging] = useState(false)
  const [mayHoldImages, setMayHoldImages] = useState(true)
  const [mayBeLink, setMayBeLink] = useState(false)
  const depthRef = useRef(0)
  const scopeRef = useRef(scope)
  scopeRef.current = scope
  const mountedRef = useRef(true)

  useEffect(() => {
    trackInternalDrags(window)
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  const onDragEnter = useCallback(
    (event: DragEvent<HTMLElement>) => {
      if (!enabled || !carriesFiles(event.dataTransfer)) return
      depthRef.current += 1
      setMayHoldImages(dragMayHoldImages(event.dataTransfer))
      setMayBeLink(!Array.from(event.dataTransfer.types ?? []).includes('Files'))
      setDragging(true)
    },
    [enabled],
  )

  const onDragOver = useCallback(
    (event: DragEvent<HTMLElement>) => {
      if (!enabled || !carriesFiles(event.dataTransfer)) return
      // Accepting the drag is what makes this a drop target; otherwise the
      // webview opens the dropped file itself.
      event.preventDefault()
      event.dataTransfer.dropEffect = 'copy'
    },
    [enabled],
  )

  const onDragLeave = useCallback(
    (event: DragEvent<HTMLElement>) => {
      if (!enabled || !carriesFiles(event.dataTransfer)) return
      depthRef.current = Math.max(0, depthRef.current - 1)
      if (depthRef.current === 0) setDragging(false)
    },
    [enabled],
  )

  const onDrop = useCallback(
    (event: DragEvent<HTMLElement>) => {
      if (!enabled || !carriesFiles(event.dataTransfer)) return
      depthRef.current = 0
      setDragging(false)
      const files = Array.from(event.dataTransfer.files ?? [])
      // A child that took the drop (the composer body placing an image inline)
      // has already prevented the default; read that before preventing it here.
      const handledByChild = event.defaultPrevented
      const zone = zoneOf ? zoneOf(event.target) : null
      if (files.length > 0) {
        event.preventDefault()
        onFiles(files, { handledByChild, zone })
        return
      }
      // Linux: addresses only.
      const addresses = droppedFileAddresses(event.dataTransfer)
      if (addresses === null) {
        // A link after all. A text field takes it as usual; anywhere else the
        // webview would open it.
        if (!isEditable(event.target)) event.preventDefault()
        return
      }
      event.preventDefault()
      // The composer body never got the files, so nothing was inlined. Go takes
      // a moment: the files still belong where they were dropped.
      const droppedOn = scopeRef.current
      void readNativeDrop(addresses).then((read) => {
        if (read.length === 0 || !mountedRef.current || scopeRef.current !== droppedOn) return
        onFiles(read, { handledByChild: false, zone })
      })
    },
    [enabled, onFiles, zoneOf],
  )

  return { dragging, mayHoldImages, mayBeLink, dropHandlers: { onDragEnter, onDragOver, onDragLeave, onDrop } }
}

const isImage = (file: File) => file.type.toLowerCase().startsWith('image/')

/** Splits a drop on the full composer: images dropped on the message body go
 *  inline (rich text only), everything else is attached. When the body's editor
 *  already placed the images itself (useComposer's handleDrop, macOS and
 *  Windows), only the other files are left. */
export function routeComposerDrop(
  files: File[],
  { inBody, rich, handledByChild }: { inBody: boolean; rich: boolean; handledByChild: boolean },
): { inline: File[]; attach: File[] } {
  if (handledByChild) return { inline: [], attach: files.filter((file) => !isImage(file)) }
  if (!inBody || !rich) return { inline: [], attach: files }
  return { inline: files.filter(isImage), attach: files.filter((file) => !isImage(file)) }
}
