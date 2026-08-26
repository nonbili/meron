import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from '../../lib/i18n'
import { Gallery, type GalleryItem } from './Gallery'
import { HtmlFrame } from './HtmlFrame'
import { prepareBubbleHtml } from './bubbleHtml'
import { applyFrameHighlights, clearFrameHighlights } from './frameSearchHighlight'
import { frameMetrics, measureFrameHeight } from './frameHeight'
import { useMessageFrameFont } from './useMessageFrameFont'

const DEFAULT_FRAME_HEIGHT = 120
const HEIGHT_CHANGE_EPSILON = 1
const FRAME_OVERSCAN = '150% 0px'
const measuredHeights = new Map<string, number>()

function cacheKeyForHtml(html: string) {
  let hash = 0
  for (let index = 0; index < html.length; index += 1) {
    hash = (Math.imul(hash, 31) + html.charCodeAt(index)) | 0
  }
  return `${html.length}:${hash}`
}

// Renders an email's HTML body in a self-sizing sandboxed iframe, wraps each
// standalone <pre> in a copy-code affordance and tracks the content height so
// the frame grows to fit while the bubble wrapper owns scrolling.
export function BubbleHtmlFrame({
  html,
  allowRemote = false,
  searchQuery = '',
  activeSearchMatch = false,
  onLinkHover,
  onUserScrollIntent,
}: {
  html: string
  /** Loosen the baked CSP so this message's remote content loads. */
  allowRemote?: boolean
  /** In-thread search query; matches are marked inside the frame document. */
  searchQuery?: string
  activeSearchMatch?: boolean
  onLinkHover?: (url: string | null) => void
  onUserScrollIntent?: () => void
}) {
  const { t } = useTranslation()
  const messageFont = useMessageFrameFont()
  // Re-preparing the document is what re-renders the frame with new typography.
  const prepareHtml = useCallback(
    (raw: string) => prepareBubbleHtml(raw, messageFont, allowRemote),
    [messageFont, allowRemote],
  )
  // Typography is part of the key: the same HTML measures to a different height
  // once the message font or text size changes.
  const cacheKey = useMemo(
    // Remote content is part of the key too: revealing it usually makes the
    // document taller, so a cached height from the blocked render is stale.
    () => `${messageFont.family ?? ''}:${messageFont.zoom}:${allowRemote}:${cacheKeyForHtml(html)}`,
    [html, messageFont, allowRemote],
  )
  const cachedHeight = measuredHeights.get(cacheKey)
  const [height, setHeight] = useState(() => cachedHeight ?? DEFAULT_FRAME_HEIGHT)
  const [measured, setMeasured] = useState(() => cachedHeight !== undefined)
  const [nearViewport, setNearViewport] = useState(() => typeof IntersectionObserver === 'undefined')
  const hostRef = useRef<HTMLDivElement | null>(null)
  const heightRef = useRef(height)
  const measuredRef = useRef(measured)
  const [frameDoc, setFrameDoc] = useState<Document | null>(null)
  const [galleryItems, setGalleryItems] = useState<GalleryItem[]>([])
  const [galleryIndex, setGalleryIndex] = useState<number | null>(null)

  const openImage = useCallback((doc: Document, img: HTMLImageElement, event: Event) => {
    event.preventDefault()
    event.stopPropagation()
    if (!img.currentSrc && !img.src) return
    const imgs = Array.from(doc.querySelectorAll<HTMLImageElement>('img')).filter((el) => {
      if (!el.currentSrc && !el.src) return false
      const w = el.getAttribute('width') || ''
      const h = el.getAttribute('height') || ''
      if ((w === '1' || w === '0') && (h === '1' || h === '0')) return false
      if (el.naturalWidth === 1 || el.naturalHeight === 1) return false
      return true
    })
    setGalleryItems(
      imgs.map((el) => ({
        src: el.currentSrc || el.src,
        filename: el.alt || el.title || 'image',
      })),
    )
    setGalleryIndex(Math.max(0, imgs.indexOf(img)))
  }, [])

  const handleFrameClick = useCallback(
    (event: MouseEvent, doc: Document) => {
      const target = event.target as Element | null
      if (!target || typeof target.closest !== 'function') return false
      const img = target.closest('img') as HTMLImageElement | null
      if (!img || !img.src) return false
      openImage(doc, img, event)
      return true
    },
    [openImage],
  )

  useEffect(() => {
    const cached = measuredHeights.get(cacheKey)
    const nextHeight = cached ?? DEFAULT_FRAME_HEIGHT
    heightRef.current = nextHeight
    measuredRef.current = cached !== undefined
    setHeight(nextHeight)
    setMeasured(cached !== undefined)
  }, [cacheKey])

  useEffect(() => {
    const host = hostRef.current
    if (!host || typeof IntersectionObserver === 'undefined') {
      setNearViewport(true)
      return
    }

    const scrollRoot = host.closest('.message-scroll')
    const observer = new IntersectionObserver(([entry]) => setNearViewport(entry?.isIntersecting ?? false), {
      root: scrollRoot,
      rootMargin: FRAME_OVERSCAN,
    })
    observer.observe(host)
    return () => observer.disconnect()
  }, [cacheKey])

  const handleReady = useCallback(
    (doc: Document) => {
      let animationFrame = 0
      let disposed = false
      // Per document: what its out-of-flow content was last seen to need.
      let overflowExtent = 0
      const cleanupFns: Array<() => void> = []

      const commitHeight = (nextHeight: number) => {
        if (disposed) return
        measuredHeights.set(cacheKey, nextHeight)
        if (!measuredRef.current) {
          measuredRef.current = true
          setMeasured(true)
        }
        if (Math.abs(nextHeight - heightRef.current) < HEIGHT_CHANGE_EPSILON) return
        heightRef.current = nextHeight
        setHeight(nextHeight)
      }

      const scheduleMeasure = () => {
        if (disposed) return
        if (animationFrame) return
        animationFrame = window.requestAnimationFrame(() => {
          animationFrame = 0
          measure()
        })
      }

      const measure = () => {
        wrapOverflowingTables()
        const measurement = measureFrameHeight(frameMetrics(doc), overflowExtent)
        overflowExtent = measurement.overflowExtent
        commitHeight(measurement.height)
      }

      // The body can't scroll sideways (the frame is `scrolling="no"` so it can
      // self-size), so anything wider than the frame would be clipped outright.
      // Give the outermost overflowing table its own horizontal scroller.
      const wrapOverflowingTables = () => {
        const limit = doc.documentElement?.clientWidth ?? 0
        if (!limit) return
        for (const table of doc.querySelectorAll<HTMLTableElement>('table')) {
          if (table.closest('.meron-table-scroll')) continue
          const rect = table.getBoundingClientRect()
          const overflowsFrame = rect.left < -1 || rect.right > limit + 1
          const overflowsItself = table.scrollWidth > table.clientWidth + 1
          if (!overflowsFrame && !overflowsItself) continue

          const wrapper = doc.createElement('div')
          wrapper.className = 'meron-table-scroll'
          table.parentNode?.insertBefore(wrapper, table)
          wrapper.appendChild(table)
        }
      }

      for (const pre of doc.querySelectorAll<HTMLPreElement>('pre')) {
        if (pre.closest('.meron-code-block')) continue
        // GitLab diff rows use one <pre> per line-content cell; wrapping each
        // one would add a copy button and block padding to every diff row.
        if (pre.closest('td.line_content, th.line_content')) continue

        const wrapper = doc.createElement('div')
        wrapper.className = 'meron-code-block'
        pre.parentNode?.insertBefore(wrapper, pre)
        wrapper.appendChild(pre)

        const button = doc.createElement('button')
        button.type = 'button'
        button.className = 'meron-copy-code'
        const copyCodeText = t('chat.copyCode')
        button.title = copyCodeText
        button.setAttribute('aria-label', copyCodeText)
        button.innerHTML = `
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect>
          <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path>
        </svg>
      `
        button.addEventListener('click', (event) => {
          event.preventDefault()
          event.stopPropagation()
          navigator.clipboard?.writeText(pre.innerText).catch(() => undefined)
        })
        wrapper.appendChild(button)
      }

      setFrameDoc(doc)
      cleanupFns.push(() => setFrameDoc((current) => (current === doc ? null : current)))

      measure()
      const observer = new ResizeObserver(measure)
      observer.observe(doc.documentElement)
      if (doc.body) observer.observe(doc.body)

      for (const image of doc.querySelectorAll<HTMLImageElement>('img')) {
        if (image.complete) continue
        image.addEventListener('load', scheduleMeasure)
        image.addEventListener('error', scheduleMeasure)
        cleanupFns.push(() => {
          image.removeEventListener('load', scheduleMeasure)
          image.removeEventListener('error', scheduleMeasure)
        })
      }

      const frameWindow = doc.defaultView
      frameWindow?.addEventListener('load', scheduleMeasure)
      frameWindow?.addEventListener('resize', scheduleMeasure)
      cleanupFns.push(() => {
        frameWindow?.removeEventListener('load', scheduleMeasure)
        frameWindow?.removeEventListener('resize', scheduleMeasure)
      })

      const shortTimer = window.setTimeout(scheduleMeasure, 100)
      const longTimer = window.setTimeout(scheduleMeasure, 500)
      const fontReady = doc.fonts?.ready.then(scheduleMeasure).catch(() => undefined)
      void fontReady

      return () => {
        disposed = true
        if (animationFrame) window.cancelAnimationFrame(animationFrame)
        observer.disconnect()
        window.clearTimeout(shortTimer)
        window.clearTimeout(longTimer)
        cleanupFns.forEach((cleanup) => cleanup())
      }
    },
    [cacheKey],
  )

  // Mark search hits in the live document. Re-runs when the query, the active
  // match, or the document itself changes; clearing on teardown keeps a frame
  // that outlives the search free of stale marks.
  useEffect(() => {
    if (!frameDoc) return
    applyFrameHighlights(frameDoc, searchQuery, activeSearchMatch)
    return () => {
      // The document is gone once the frame reloads; ignore that case.
      if (frameDoc.defaultView) clearFrameHighlights(frameDoc)
    }
  }, [frameDoc, searchQuery, activeSearchMatch])

  return (
    <>
      <div ref={hostRef} style={{ height }} className="w-full">
        {nearViewport && (
          <HtmlFrame
            html={html}
            prepareHtml={prepareHtml}
            title={t('chat.messageHtml')}
            className="block w-full border-0 bg-transparent"
            style={{ height, overflow: 'hidden', visibility: measured ? 'visible' : 'hidden' }}
            scrolling="no"
            onFrameClick={handleFrameClick}
            onReady={handleReady}
            onLinkHover={onLinkHover}
            onUserScrollIntent={onUserScrollIntent}
            forwardContextMenu
          />
        )}
      </div>
      {galleryIndex !== null && galleryItems[galleryIndex] && (
        <Gallery
          items={galleryItems}
          index={galleryIndex}
          onIndexChange={setGalleryIndex}
          onClose={() => setGalleryIndex(null)}
        />
      )}
    </>
  )
}
