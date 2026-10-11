import { forwardRef, useCallback, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef } from 'react'
import type { CSSProperties, Ref } from 'react'
import { clearMediaSession } from '../../lib/mediaSession'
import { chordFromEvent } from '../../lib/shortcuts'
import { openExternal } from '../../lib/native'
import { trackInternalDrags } from '../../lib/useFileDrop'

const EXTERNAL_PROTOCOLS = new Set(['http:', 'https:', 'mailto:', 'tel:'])

type FrameClickHandler = (event: MouseEvent, doc: Document) => boolean | void
type FrameReadyHandler = (doc: Document, iframe: HTMLIFrameElement) => void | (() => void)

interface HtmlFrameProps {
  html: string
  title: string
  className?: string
  style?: CSSProperties
  scrolling?: 'auto' | 'yes' | 'no'
  prepareHtml?: (html: string) => string
  onFrameClick?: FrameClickHandler
  onReady?: FrameReadyHandler
  onLinkHover?: (url: string | null) => void
  onUserScrollIntent?: () => void
  onScroll?: () => void
  // When set, right-clicks inside the frame are blocked from showing WebKit's
  // native menu and re-dispatched as a `contextmenu` event on the iframe
  // element, so a custom menu registered on a parent element fires instead.
  forwardContextMenu?: boolean
  // Call `onReady` as soon as the document is parsed instead of waiting for
  // its `load` event, which waits for every picture: one slow image would
  // otherwise hold back whatever `onReady` does. For handlers that cope with
  // pictures arriving afterwards.
  readyWhenParsed?: boolean
}

function anchorUrl(anchor: HTMLAnchorElement): string | null {
  const rawHref = anchor.getAttribute('href') ?? ''
  if (!rawHref || rawHref.startsWith('#')) return null

  let url: URL
  try {
    url = new URL(rawHref, anchor.ownerDocument.baseURI)
  } catch {
    return null
  }
  if (!EXTERNAL_PROTOCOLS.has(url.protocol)) return null

  return url.href
}

function openAnchor(anchor: HTMLAnchorElement, event: MouseEvent) {
  const href = anchorUrl(anchor)
  if (!href) return

  event.preventDefault()
  event.stopPropagation()
  openExternal(href)
}

// Pause and unload every media element in a frame document. WebKitGTK keeps a
// GStreamer pipeline (and its MPRIS "now playing" notification) alive for any
// <video>/<audio> that gets destroyed while still playing — e.g. when an RSS
// thread with embedded video is closed. Detaching the source and calling load()
// tears the pipeline down so the lingering notification clears.
function stopFrameMedia(doc: Document | null | undefined) {
  if (!doc) return
  doc.querySelectorAll<HTMLMediaElement>('video, audio').forEach((media) => {
    try {
      media.pause()
      media.querySelectorAll('source').forEach((source) => source.remove())
      media.removeAttribute('src')
      media.load()
    } catch {
      // Ignore documents that are mid-teardown.
    }
  })
  doc.querySelectorAll('iframe').forEach((iframe) => {
    try {
      iframe.removeAttribute('src')
      iframe.src = 'about:blank'
      iframe.remove()
    } catch {
      // Ignore
    }
  })
  clearMediaSession(doc.defaultView)
}

function isEditableFrameTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null
  if (!el) return false
  return el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable
}

function hasFrameSelection(doc: Document): boolean {
  return !!doc.getSelection()?.toString().trim()
}

export const HtmlFrame = forwardRef(function HtmlFrame(
  {
    html,
    title,
    className,
    style,
    scrolling,
    prepareHtml,
    onFrameClick,
    onReady,
    onLinkHover,
    onUserScrollIntent,
    onScroll,
    forwardContextMenu,
    readyWhenParsed,
  }: HtmlFrameProps,
  forwardedRef: Ref<HTMLIFrameElement>,
) {
  const iframeRef = useRef<HTMLIFrameElement | null>(null)
  const docRef = useRef<Document | null>(null)
  const winRef = useRef<Window | null>(null)
  const { srcDoc, documentMarker } = useMemo(() => {
    const marker = `meron-frame-${Math.random().toString(36).slice(2)}`
    const prepared = prepareHtml ? prepareHtml(html) : html
    // A leading comment identifies even an already-parsed document without
    // parsing the sender's markup again or executing code inside the frame.
    return { srcDoc: `<!--${marker}-->${prepared}`, documentMarker: marker }
  }, [html, prepareHtml])

  useImperativeHandle(forwardedRef, () => iframeRef.current as HTMLIFrameElement, [])

  const cleanupReadyRef = useRef<(() => void) | undefined>(undefined)
  // The parsed document `onReady` last ran for.
  const readyDocRef = useRef<Document | null>(null)
  // Whether that run already saw the document with its pictures loaded.
  const readyLoadedRef = useRef(false)
  const readyWhenParsedRef = useRef(readyWhenParsed)
  readyWhenParsedRef.current = readyWhenParsed
  const activeScrollListenerRef = useRef<{ win: Window; listener: () => void } | null>(null)

  const onFrameClickRef = useRef(onFrameClick)
  const onReadyRef = useRef(onReady)
  const onLinkHoverRef = useRef(onLinkHover)
  const onScrollRef = useRef(onScroll)
  const onUserScrollIntentRef = useRef(onUserScrollIntent)
  const forwardContextMenuRef = useRef(forwardContextMenu)

  useEffect(() => {
    onFrameClickRef.current = onFrameClick
    onReadyRef.current = onReady
    onLinkHoverRef.current = onLinkHover
    onScrollRef.current = onScroll
    onUserScrollIntentRef.current = onUserScrollIntent
    forwardContextMenuRef.current = forwardContextMenu
  }, [onFrameClick, onReady, onLinkHover, onScroll, onUserScrollIntent, forwardContextMenu])

  const wire = useCallback((atLoad = false) => {
    const iframe = iframeRef.current
    if (!iframe) return

    const doc = iframe.contentDocument
    const win = iframe.contentWindow
    if (!doc || !win) return
    // A body can exist while srcdoc is still parsing: ready only after that.
    if (doc.readyState === 'loading') return
    // Ready once per document, so neither a repeated setup nor a new srcDoc
    // whose document has not replaced this one yet runs the hook again. The
    // exception is `load` for a reader that did not ask for parsed readiness
    // and was readied before its pictures were in: scroll restoration needs
    // the final layout.
    if (readyDocRef.current === doc && (!atLoad || readyWhenParsedRef.current || readyLoadedRef.current)) return

    docRef.current = doc
    winRef.current = win
    // A link or image dragged out of the message is not a file drop.
    trackInternalDrags(doc)
    if (!doc.documentElement.dataset.meronFrameScrollIntentWired) {
      doc.documentElement.dataset.meronFrameScrollIntentWired = '1'
      doc.addEventListener('wheel', () => onUserScrollIntentRef.current?.(), { passive: true })
      doc.addEventListener('touchmove', () => onUserScrollIntentRef.current?.(), { passive: true })
      doc.addEventListener('keydown', (event) => {
        if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(event.key)) {
          onUserScrollIntentRef.current?.()
        }
      })
    }

    // Clean up previous ready hook if any
    cleanupReadyRef.current?.()
    cleanupReadyRef.current = undefined

    // Clean up previous scroll listener
    if (activeScrollListenerRef.current) {
      const { win: oldWin, listener: oldListener } = activeScrollListenerRef.current
      try {
        oldWin.removeEventListener('scroll', oldListener)
      } catch {
        // Ignore if window was already destroyed
      }
      activeScrollListenerRef.current = null
    }

    if (!doc.documentElement.dataset.meronFrameLinkWired) {
      doc.documentElement.dataset.meronFrameLinkWired = '1'
      const handleClick = (event: MouseEvent) => {
        if (event.button === 2) return
        if (onFrameClickRef.current?.(event, doc)) return

        const target = event.target as Element | null
        const anchor = target?.closest?.('a[href]') as HTMLAnchorElement | null
        if (anchor) openAnchor(anchor, event)
      }
      doc.addEventListener('click', handleClick, true)
      doc.addEventListener('auxclick', handleClick, true)
    }

    if (!doc.documentElement.dataset.meronFrameLinkHoverWired) {
      doc.documentElement.dataset.meronFrameLinkHoverWired = '1'
      const handleLinkEnter = (event: MouseEvent | FocusEvent) => {
        const target = event.target as Element | null
        const anchor = target?.closest?.('a[href]') as HTMLAnchorElement | null
        onLinkHoverRef.current?.(anchor ? anchorUrl(anchor) : null)
      }
      const handleLinkLeave = (event: MouseEvent | FocusEvent) => {
        const target = event.target as Element | null
        const anchor = target?.closest?.('a[href]') as HTMLAnchorElement | null
        if (!anchor) return
        const related = 'relatedTarget' in event ? (event.relatedTarget as Node | null) : null
        if (related && anchor.contains(related)) return
        onLinkHoverRef.current?.(null)
      }
      doc.addEventListener('mouseover', handleLinkEnter, true)
      doc.addEventListener('mouseout', handleLinkLeave, true)
      doc.addEventListener('focusin', handleLinkEnter, true)
      doc.addEventListener('focusout', handleLinkLeave, true)
    }

    if (!doc.documentElement.dataset.meronFrameContextWired) {
      doc.documentElement.dataset.meronFrameContextWired = '1'
      doc.addEventListener('contextmenu', (event) => {
        if (!forwardContextMenuRef.current) return
        if (hasFrameSelection(doc)) return

        const target = event.target as Element | null
        const anchor = target?.closest?.('a[href]') as HTMLAnchorElement | null
        let linkUrl: string | undefined = undefined
        if (anchor) {
          const rawHref = anchor.getAttribute('href') ?? ''
          if (rawHref && !rawHref.startsWith('#')) {
            try {
              const url = new URL(rawHref, anchor.ownerDocument.baseURI)
              if (EXTERNAL_PROTOCOLS.has(url.protocol)) {
                linkUrl = url.href
              }
            } catch {
              // Ignore invalid url
            }
          }
        }

        // Block WebKit's native frame menu and re-fire on the iframe element so
        // a parent-registered onContextMenu (e.g. the message menu) handles it.
        event.preventDefault()
        const frame = iframeRef.current
        if (!frame) return
        const rect = frame.getBoundingClientRect()
        const customEvent = new MouseEvent('contextmenu', {
          bubbles: true,
          cancelable: true,
          clientX: rect.left + event.clientX,
          clientY: rect.top + event.clientY,
        })
        if (linkUrl) {
          ;(customEvent as any).meronLinkUrl = linkUrl
        }
        frame.dispatchEvent(customEvent)
      })
    }

    if (!doc.documentElement.dataset.meronFrameKeyWired) {
      doc.documentElement.dataset.meronFrameKeyWired = '1'
      doc.addEventListener('keydown', (event) => {
        // Forward the chords the app can act on: ⌘/Ctrl (or Alt) shortcuts such
        // as ⌘/Ctrl+F, which the parent's keydown listener never sees while
        // focus is in the frame, plus bare Up/Down for list navigation.
        const chord = chordFromEvent(event)
        if (!chord) return
        const isBareArrow =
          !chord.mod && !chord.alt && !chord.shift && (chord.key === 'ArrowDown' || chord.key === 'ArrowUp')
        if (!chord.mod && !chord.alt && !isBareArrow) return
        // Only the unmodified arrows must stand down for a field inside the
        // message (they move the caret); ⌘/Ctrl chords still belong to the app.
        if (isBareArrow && isEditableFrameTarget(event.target)) return

        const forwarded = new CustomEvent('meron.frameKeyDown', {
          cancelable: true,
          detail: chord,
        })
        if (!window.dispatchEvent(forwarded) || forwarded.defaultPrevented) {
          event.preventDefault()
        }
      })
    }

    if (onScrollRef.current) {
      const listener = () => onScrollRef.current?.()
      win.addEventListener('scroll', listener, { passive: true })
      activeScrollListenerRef.current = { win, listener }
    }

    if (!doc.documentElement.dataset.meronFrameMediaUnloadWired) {
      doc.documentElement.dataset.meronFrameMediaUnloadWired = '1'
      win.addEventListener('unload', () => {
        stopFrameMedia(doc)
      })
    }

    cleanupReadyRef.current = onReadyRef.current?.(doc, iframe) ?? undefined
    readyDocRef.current = doc
    readyLoadedRef.current = atLoad || doc.readyState === 'complete'
  }, [])

  // Listen to native load events which are guaranteed to fire when srcDoc loads
  useEffect(() => {
    const iframe = iframeRef.current
    if (!iframe) return

    // Wire immediately on mount (in case it already loaded)
    wire()

    const onLoad = () => wire(true)
    iframe.addEventListener('load', onLoad)

    return () => {
      iframe.removeEventListener('load', onLoad)
    }
  }, [wire])

  // Wire when srcDoc changes to cover cases where the load event might not fire
  useEffect(() => {
    wire()
  }, [wire, srcDoc])

  // Discover the committed srcdoc document, then wait for its parsing event.
  // Polling ends at navigation commit, so hanging images cannot keep it running.
  useEffect(() => {
    const iframe = iframeRef.current
    if (!readyWhenParsed || !iframe) return
    let watched: Document | null = null
    let frame = 0
    const parsed = () => wire()
    const watch = () => {
      const doc = iframe.contentDocument
      // The old document can linger for several frames; the current marker
      // also lets an already-parsed document settle immediately on remount.
      if (!doc || doc.firstChild?.nodeValue !== documentMarker) {
        frame = window.requestAnimationFrame(watch)
        return
      }
      watched = doc
      if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', parsed, { once: true })
      else wire()
    }
    watch()
    return () => {
      window.cancelAnimationFrame(frame)
      watched?.removeEventListener('DOMContentLoaded', parsed)
    }
  }, [wire, srcDoc, documentMarker, readyWhenParsed])

  // Stop in-frame media before the document is swapped for new HTML, otherwise
  // its orphaned pipeline keeps a "now playing" notification up.
  useLayoutEffect(() => {
    return () => {
      stopFrameMedia(docRef.current)
      clearMediaSession(winRef.current)
    }
  }, [srcDoc])

  // Cleanup on unmount
  useLayoutEffect(() => {
    return () => {
      stopFrameMedia(docRef.current)
      clearMediaSession(winRef.current)
      cleanupReadyRef.current?.()
      cleanupReadyRef.current = undefined
      // StrictMode replays setup on this same document after disposing its
      // installed hook. Let setup reinstall it; a `load` with nothing to add
      // still sees an active readyDocRef and is ignored.
      readyDocRef.current = null

      if (activeScrollListenerRef.current) {
        const { win, listener } = activeScrollListenerRef.current
        try {
          win.removeEventListener('scroll', listener)
        } catch {
          // Ignore
        }
        activeScrollListenerRef.current = null
      }
    }
  }, [])

  return (
    <iframe
      ref={iframeRef}
      srcDoc={srcDoc}
      // `allow-scripts` is required so our parent-registered click listener
      // actually fires: WebKitGTK disables *all* script execution in a
      // scripting-sandboxed document, including listeners added from the parent
      // realm, which left links unclickable. Email JS is still blocked by the
      // `script`-less CSP that every rendered document carries (`prepare_html`
      // for the reader, `prepareBubbleHtml` for the bubble).
      sandbox="allow-same-origin allow-scripts"
      title={title}
      className={className}
      style={style}
      scrolling={scrolling}
    />
  )
})
