import { forwardRef, useCallback, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef } from 'react'
import type { CSSProperties, Ref } from 'react'
import { clearMediaSession } from '../../lib/mediaSession'
import { openExternal } from '../../lib/native'

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
  onScroll?: () => void
  // When set, right-clicks inside the frame are blocked from showing WebKit's
  // native menu and re-dispatched as a `contextmenu` event on the iframe
  // element, so a custom menu registered on a parent element fires instead.
  forwardContextMenu?: boolean
}

function openAnchor(anchor: HTMLAnchorElement, event: MouseEvent) {
  const rawHref = anchor.getAttribute('href') ?? ''
  if (!rawHref || rawHref.startsWith('#')) return

  let url: URL
  try {
    url = new URL(rawHref, anchor.ownerDocument.baseURI)
  } catch {
    return
  }
  if (!EXTERNAL_PROTOCOLS.has(url.protocol)) return

  event.preventDefault()
  event.stopPropagation()
  openExternal(url.href)
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
    onScroll,
    forwardContextMenu,
  }: HtmlFrameProps,
  forwardedRef: Ref<HTMLIFrameElement>,
) {
  const iframeRef = useRef<HTMLIFrameElement | null>(null)
  const docRef = useRef<Document | null>(null)
  const winRef = useRef<Window | null>(null)
  const srcDoc = useMemo(() => (prepareHtml ? prepareHtml(html) : html), [html, prepareHtml])

  useImperativeHandle(forwardedRef, () => iframeRef.current as HTMLIFrameElement, [])

  const cleanupReadyRef = useRef<(() => void) | undefined>(undefined)
  const activeScrollListenerRef = useRef<{ win: Window; listener: () => void } | null>(null)

  const onFrameClickRef = useRef(onFrameClick)
  const onReadyRef = useRef(onReady)
  const onScrollRef = useRef(onScroll)
  const forwardContextMenuRef = useRef(forwardContextMenu)

  useEffect(() => {
    onFrameClickRef.current = onFrameClick
    onReadyRef.current = onReady
    onScrollRef.current = onScroll
    forwardContextMenuRef.current = forwardContextMenu
  }, [onFrameClick, onReady, onScroll, forwardContextMenu])

  const wire = useCallback(() => {
    const iframe = iframeRef.current
    if (!iframe) return

    const doc = iframe.contentDocument
    const win = iframe.contentWindow
    if (!doc || !win) return

    docRef.current = doc
    winRef.current = win

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
        if (
          (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') ||
          event.metaKey ||
          event.ctrlKey ||
          event.altKey ||
          event.shiftKey ||
          isEditableFrameTarget(event.target)
        ) {
          return
        }
        const forwarded = new CustomEvent('meron.frameKeyDown', {
          cancelable: true,
          detail: { key: event.key },
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
  }, [])

  // Listen to native load events which are guaranteed to fire when srcDoc loads
  useEffect(() => {
    const iframe = iframeRef.current
    if (!iframe) return

    // Wire immediately on mount (in case it already loaded)
    wire()

    iframe.addEventListener('load', wire)

    return () => {
      iframe.removeEventListener('load', wire)
    }
  }, [wire])

  // Wire when srcDoc changes to cover cases where the load event might not fire
  useEffect(() => {
    wire()
  }, [wire, srcDoc])

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
