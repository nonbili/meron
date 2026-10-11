import { useCallback, useEffect, useRef, useState } from 'react'
import type { DragEvent } from 'react'
import { pickedToFile, type PickedFile } from './nativeFilePicker'

/** Which drop area the drop landed in (from `zoneOf`, decided at the drop), and
 *  whether a child (the composer body's editor) already took part of it. */
export type FileDropInfo = { handledByChild: boolean; zone: string | null }
type FilesHandler = (files: File[], drop: FileDropInfo) => void

// A drag that starts inside the page (a link or an image in a message) is never
// a file drop. Cards and columns use pointer events, not HTML drag and drop.
let internalDrag = false
let internalDragListening = false

function listenForInternalDrags() {
  if (internalDragListening || typeof window === 'undefined') return
  internalDragListening = true
  window.addEventListener('dragstart', () => (internalDrag = true), true)
  window.addEventListener('dragend', () => (internalDrag = false), true)
}

/** Whether a drag carries files from outside the page (the file manager). macOS
 *  and Windows announce them as `Files`. WebKitGTK on Linux announces a file from
 *  the file manager only as its file:// address (`text/uri-list`); the file
 *  itself then comes from Go (filedrop.go, deliverNativeDrop). */
function carriesFiles(data: DataTransfer | null | undefined): boolean {
  if (internalDrag || !data) return false
  const types = Array.from(data.types ?? [])
  return types.includes('Files') || types.includes('text/uri-list')
}

/** A Linux file manager drop: file addresses but no files, which arrive from Go
 *  instead. The composer body's editor must not paste the address as text. */
export function isExternalFileDrop(data: DataTransfer): boolean {
  return carriesFiles(data) && !(data.files && data.files.length > 0)
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

// On Linux the page's drop event and Go's files.dropped event arrive in either
// order; whichever comes second completes the drop, if within this window.
const NATIVE_DROP_WINDOW_MS = 3000
let pendingDrop: { onFiles: FilesHandler; zone: string | null; at: number } | null = null
let pendingFiles: { files: File[]; at: number } | null = null
let nativeDropListening = false

/** Files Go read from a native drop (Linux). They go to the drop area the drop
 *  event landed on; the composer body never got them, so nothing was inlined. */
export function deliverNativeDrop(files: File[]) {
  const now = Date.now()
  if (pendingDrop && now - pendingDrop.at <= NATIVE_DROP_WINDOW_MS) {
    const { onFiles, zone } = pendingDrop
    pendingDrop = null
    onFiles(files, { handledByChild: false, zone })
    return
  }
  pendingDrop = null
  pendingFiles = { files, at: now }
}

function awaitNativeDrop(onFiles: FilesHandler, zone: string | null) {
  const now = Date.now()
  if (pendingFiles && now - pendingFiles.at <= NATIVE_DROP_WINDOW_MS) {
    const { files } = pendingFiles
    pendingFiles = null
    onFiles(files, { handledByChild: false, zone })
    return
  }
  pendingFiles = null
  pendingDrop = { onFiles, zone, at: now }
}

function listenForNativeDrops() {
  if (nativeDropListening) return
  const eventsOn = (window as any).runtime?.EventsOn
  if (typeof eventsOn !== 'function') return
  nativeDropListening = true
  eventsOn('files.dropped', (payload: { files?: PickedFile[] }) => {
    const files = (payload?.files ?? []).map(pickedToFile)
    if (files.length > 0) deliverNativeDrop(files)
  })
}

export function resetFileDropStateForTests() {
  internalDrag = false
  pendingDrop = null
  pendingFiles = null
}

/**
 * Lets an element take files dropped onto it. `dragging` is true while files are
 * held over the element, for a drop overlay, and `mayHoldImages` says whether
 * they might be images; `onFiles` receives what was dropped and where: `zoneOf`
 * names the drop area from the drop's target, at the drop itself, since on Linux
 * the files arrive only after the overlay they were dropped on is gone.
 *
 * Every other drag passes through untouched. Child elements fire their own
 * enter/leave pairs as the pointer moves across them, so a depth count keeps the
 * overlay from flickering.
 */
export function useFileDrop(
  onFiles: FilesHandler,
  { enabled = true, zoneOf }: { enabled?: boolean; zoneOf?: (target: EventTarget | null) => string | null } = {},
) {
  const [dragging, setDragging] = useState(false)
  const [mayHoldImages, setMayHoldImages] = useState(true)
  const depthRef = useRef(0)

  useEffect(() => {
    listenForInternalDrags()
    listenForNativeDrops()
  }, [])

  const onDragEnter = useCallback(
    (event: DragEvent<HTMLElement>) => {
      if (!enabled || !carriesFiles(event.dataTransfer)) return
      depthRef.current += 1
      setMayHoldImages(dragMayHoldImages(event.dataTransfer))
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
      event.preventDefault()
      if (files.length > 0) onFiles(files, { handledByChild, zone })
      else awaitNativeDrop(onFiles, zone)
    },
    [enabled, onFiles, zoneOf],
  )

  return { dragging, mayHoldImages, dropHandlers: { onDragEnter, onDragOver, onDragLeave, onDrop } }
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
