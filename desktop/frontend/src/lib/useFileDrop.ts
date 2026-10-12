import { useCallback, useEffect, useRef, useState } from 'react'
import type { DragEvent } from 'react'
import { invoke } from './bridge'
import { reportUnattachedFiles } from './attachFiles'
import { pickedToFile, type PickedFile } from './nativeFilePicker'

/** Which drop area the drop landed in (from `zoneOf`, decided at the drop), and
 *  whether a child (the composer body's editor) already took part of it. */
export type FileDropInfo = { handledByChild: boolean; zone: string | null }
type FilesHandler = (files: File[], drop: FileDropInfo) => void

// A drag that starts inside the page (a link or an image in a message) is never
// a file drop. Cards and columns use pointer events, not HTML drag and drop.
let internalDrag = false
const internalDragTargets = new WeakSet<Window | Document>()

// What the drop areas (useFileDrop) need to hear from the documents watched
// below: a file drag over a message frame, whose events never reach the pane
// around it, and that no drag is under way any more.
const frameDragListeners = new Set<(frame: Element, data: DataTransfer) => void>()
const dragOverListeners = new Set<() => void>()
let fileDragSeen = false

/** Watches a document's drags. The page's own window is watched by useFileDrop;
 *  message bodies live in iframes, whose drag events never reach it, so
 *  HtmlFrame adds each frame's document, with the `frame` element showing it.
 *
 *  Drags that start in it are marked internal. File drags over anything that is
 *  not a drop area (the handlers below accept those first) are refused, or the
 *  webview would open the dropped file in place of the app. */
export function trackInternalDrags(target: Window | Document, frame?: Element | null) {
  if (internalDragTargets.has(target)) return
  internalDragTargets.add(target)
  const ended = () => (internalDrag = false)
  target.addEventListener('dragstart', () => (internalDrag = true), true)
  target.addEventListener('dragend', ended, true)
  // `dragend` is lost when the dragged element is removed mid-drag, and a drag
  // that leaves the window may report nothing more. A mouse that moves or is
  // pressed with no button held from before says no drag is under way: the
  // mark is cleared, and drop areas put their overlays away.
  target.addEventListener('mousedown', ended, true)
  target.addEventListener(
    'mousemove',
    (event) => {
      if ((event as MouseEvent).buttons !== 0) return
      internalDrag = false
      if (!fileDragSeen) return
      fileDragSeen = false
      dragOverListeners.forEach((listener) => listener())
    },
    { capture: true, passive: true },
  )
  if (frame) {
    const overFrame = (event: Event) => {
      const data = (event as globalThis.DragEvent).dataTransfer
      if (data && carriesFiles(data)) frameDragListeners.forEach((listener) => listener(frame, data))
    }
    target.addEventListener('dragenter', overFrame, true)
    target.addEventListener('dragover', overFrame, true)
  }
  const refuse = (event: Event) => {
    const drag = event as globalThis.DragEvent
    if (drag.defaultPrevented || !carriesFiles(drag.dataTransfer)) return
    // A Linux drag of addresses may be a link, which a text field takes.
    const mayBeLink = !Array.from(drag.dataTransfer?.types ?? []).includes('Files')
    if (mayBeLink && isEditable(drag.target)) return
    drag.preventDefault()
    if (drag.type === 'dragover' && drag.dataTransfer) drag.dataTransfer.dropEffect = 'none'
  }
  target.addEventListener('dragover', refuse)
  // After `refuse`, which still needs to see the drop as internal. Not
  // capturing either, for the drop handlers below.
  target.addEventListener('drop', refuse)
  target.addEventListener('drop', ended)
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
    const result = await invoke<{ files?: PickedFile[]; skipped?: number }>('system.readDroppedFiles', {
      uris: addresses,
    })
    if ((result?.skipped ?? 0) > 0) reportUnattachedFiles()
    return (result?.files ?? []).map(pickedToFile)
  } catch (error) {
    console.error('Failed to read dropped files:', error)
    reportUnattachedFiles()
    return []
  }
}

function isEditable(target: EventTarget | null): boolean {
  // Not `instanceof Element`: a target in a message frame is of another realm.
  const closest = (target as Element | null)?.closest
  if (typeof closest !== 'function') return false
  return !!closest.call(target, 'input, textarea, [contenteditable]:not([contenteditable="false"])')
}

/** Whether a drag left the element: the pointer of a `dragleave` is outside it.
 *  Leaves fired while crossing its children are inside. */
function leftElement(event: DragEvent<HTMLElement>): boolean {
  const element = event.currentTarget
  if (typeof element?.getBoundingClientRect !== 'function' || typeof event.clientX !== 'number') return false
  const box = element.getBoundingClientRect()
  return (
    event.clientX <= box.left || event.clientX >= box.right || event.clientY <= box.top || event.clientY >= box.bottom
  )
}

export function resetFileDropStateForTests(options: { fileAddressDrops?: boolean } = {}) {
  internalDrag = false
  fileDragSeen = false
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
 * overlay from flickering. The count alone can be left too high (a child removed
 * mid-drag never reports its leave), so a leave outside the element, and a mouse
 * that moves with no drag under way, also put the overlay away. `dropHandlers`
 * carries a ref to the element: a file drag over a message frame inside it
 * reports to the frame's document only (trackInternalDrags).
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
  // Read when the drop is handled, which on Linux is after Go answered: the
  // caller's handler of that moment, not of the render the drop happened in.
  const onFilesRef = useRef(onFiles)
  onFilesRef.current = onFiles
  const zoneOfRef = useRef(zoneOf)
  zoneOfRef.current = zoneOf
  const mountedRef = useRef(true)
  const enabledRef = useRef(enabled)
  enabledRef.current = enabled
  const elementRef = useRef<HTMLElement | null>(null)
  const ref = useCallback((element: HTMLElement | null) => {
    elementRef.current = element
  }, [])

  const show = useCallback((data: DataTransfer) => {
    fileDragSeen = true
    setMayHoldImages(dragMayHoldImages(data))
    setMayBeLink(!Array.from(data.types ?? []).includes('Files'))
    setDragging(true)
  }, [])

  useEffect(() => {
    trackInternalDrags(window)
    mountedRef.current = true
    const overFrame = (frame: Element, data: DataTransfer) => {
      if (enabledRef.current && elementRef.current?.contains(frame)) show(data)
    }
    const dragOver = () => {
      depthRef.current = 0
      setDragging(false)
    }
    frameDragListeners.add(overFrame)
    dragOverListeners.add(dragOver)
    return () => {
      mountedRef.current = false
      frameDragListeners.delete(overFrame)
      dragOverListeners.delete(dragOver)
    }
  }, [show])

  // Turned off mid-drag (a reader tab opened in front, the conversation went
  // away): no leave or drop will be taken to put the overlay away.
  useEffect(() => {
    if (enabled) return
    depthRef.current = 0
    setDragging(false)
  }, [enabled])

  const onDragEnter = useCallback(
    (event: DragEvent<HTMLElement>) => {
      if (!enabled || !carriesFiles(event.dataTransfer)) return
      depthRef.current += 1
      show(event.dataTransfer)
    },
    [enabled, show],
  )

  const onDragOver = useCallback(
    (event: DragEvent<HTMLElement>) => {
      if (!enabled || !carriesFiles(event.dataTransfer)) return
      // Accepting the drag is what makes this a drop target; otherwise the
      // webview opens the dropped file itself.
      event.preventDefault()
      event.dataTransfer.dropEffect = 'copy'
      // Also when the enter was missed: the drag came in over a message frame.
      show(event.dataTransfer)
    },
    [enabled, show],
  )

  const onDragLeave = useCallback(
    (event: DragEvent<HTMLElement>) => {
      if (!enabled || !carriesFiles(event.dataTransfer)) return
      depthRef.current = leftElement(event) ? 0 : Math.max(0, depthRef.current - 1)
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
      const zone = zoneOfRef.current ? zoneOfRef.current(event.target) : null
      if (files.length > 0) {
        event.preventDefault()
        onFilesRef.current(files, { handledByChild, zone })
        return
      }
      if (!fileAddressDrops) {
        // Files announced but none handed over (an item another app only
        // promises, on macOS). Asking Go is for Linux.
        event.preventDefault()
        reportUnattachedFiles()
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
        if (read.length === 0) return
        // No longer what they were dropped on: not attached, and said so.
        // Nor still a drop area: something opened in front of it meanwhile.
        if (!mountedRef.current || !enabledRef.current || scopeRef.current !== droppedOn) {
          return reportUnattachedFiles()
        }
        onFilesRef.current(read, { handledByChild: false, zone })
      })
    },
    [enabled],
  )

  return { dragging, mayHoldImages, mayBeLink, dropHandlers: { ref, onDragEnter, onDragOver, onDragLeave, onDrop } }
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
