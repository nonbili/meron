import { useValue } from '@legendapp/state/react'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from '../../lib/i18n'
import { copyText } from '../../lib/native'
import { Gallery, type GalleryItem } from './Gallery'
import { HtmlFrame } from './HtmlFrame'
import {
  FRAME_GENERATION_MARKER,
  applyBubbleTheme,
  applyBubbleThemeAsync,
  prepareBubbleHtml,
  reloadFailedImages,
  reserveImageBox,
  releaseFailedImageBox,
  restoreImageBox,
} from './bubbleHtml'
import { bodyContentKey } from './messageHelpers'
import { applyFrameHighlights, clearFrameHighlights } from './frameSearchHighlight'
import { frameMetrics, measureFrameHeight } from './frameHeight'
import { useMessageFrameFont } from './useMessageFrameFont'
import { installFrameQuoteFold, isInFoldedQuote } from './quoteFold'
import { useBubbleTheme } from './useFrameTheme'
import type { FrameSurface } from './frameTheme'
import { settings$ } from '../../states/settings'

const DEFAULT_FRAME_HEIGHT = 120
const HEIGHT_CHANGE_EPSILON = 1
const FRAME_OVERSCAN = '150% 0px'
const measuredHeights = new Map<string, number>()
// What each document last reported to onNaturalWidth, so a remounted bubble takes
// its width before it first paints instead of flashing at full width.
const naturalWidths = new Map<string, number | null>()
// Anything whose layout depends on the width it is given rather than on its text.
const NON_TEXT_SELECTOR = 'table, img, video, iframe, svg, pre, [width], [style*="width"]'

// Renders an email's HTML body in a self-sizing sandboxed iframe, wraps each
// standalone <pre> in a copy-code affordance and tracks the content height so
// the frame grows to fit while the bubble wrapper owns scrolling.
export function BubbleHtmlFrame({
  html,
  outgoing = false,
  surface = 'bubble',
  allowRemote = false,
  mediaMissing = 0,
  searchQuery = '',
  activeSearchOffset = -1,
  onLinkHover,
  onUserScrollIntent,
  onNaturalWidth,
}: {
  html: string
  /** Which bubble the frame sits in: its colors are the ones the frame paints with. */
  outgoing?: boolean
  /** A chat bubble or a traditional layout's row: what a darkened body blends onto. */
  surface?: FrameSurface
  /** Loosen the baked CSP so this message's remote content loads. */
  allowRemote?: boolean
  /** How many attachment files this message refers to are not on disk yet. */
  mediaMissing?: number
  /** In-thread search query; matches are marked inside the frame document. */
  searchQuery?: string
  /** Which of this frame's matches the search is parked on, -1 for none. */
  activeSearchOffset?: number
  onLinkHover?: (url: string | null) => void
  onUserScrollIntent?: () => void
  /** Reports the width a text-only document needs, or null for one that should fill the bubble. */
  onNaturalWidth?: (width: number | null) => void
}) {
  const { t } = useTranslation()
  const messageFont = useMessageFrameFont()
  const bubbleTheme = useBubbleTheme(outgoing, surface)
  const autoFit = useValue(settings$.autoFitMessages)
  const onNaturalWidthRef = useRef(onNaturalWidth)
  onNaturalWidthRef.current = onNaturalWidth
  const autoFitRef = useRef(autoFit)
  autoFitRef.current = autoFit
  // Everything the frame's document is built from. Typography is part of it:
  // the same HTML measures to a different height once the message font or text
  // size changes, and so is remote content, since revealing it usually makes the
  // document taller, and fitting wide tables, which makes it shorter. A change
  // here reloads the frame.
  const documentKey = useMemo(
    () => `${messageFont.family ?? ''}:${messageFont.zoom}:${allowRemote}:${autoFit}:${bodyContentKey(html)}`,
    [html, messageFont, allowRemote, autoFit],
  )
  // What this document is called while it is the current one. The frame is wired
  // as soon as its srcDoc changes — while the document it replaces is still
  // loaded — so the ready handler checks the stamp before it measures anything,
  // and leaves the outgoing document to the handler it already has.
  const generation = useMemo(() => `${documentKey.length}-${Math.random().toString(36).slice(2)}`, [documentKey])
  // Re-preparing the document is what re-renders the frame with new typography.
  // The theme isn't baked in: its decision needs the rendered document, so it is
  // applied to the live frame instead (see applyBubbleTheme).
  const prepareHtml = useCallback(
    (raw: string) => prepareBubbleHtml(raw, messageFont, allowRemote, generation),
    [messageFont, allowRemote, generation],
  )
  // The appearance changes the height too — a self-styled message gets a padded
  // card in a dark theme — but repaints the live document instead of reloading
  // it, so it is the one dimension a measurement reads live.
  const appearanceRef = useRef(bubbleTheme.appearance)
  appearanceRef.current = bubbleTheme.appearance
  const cacheKey = `${documentKey}:${bubbleTheme.appearance}`
  const cachedHeight = measuredHeights.get(cacheKey)
  const [height, setHeight] = useState(() => cachedHeight ?? DEFAULT_FRAME_HEIGHT)
  const [nearViewport, setNearViewport] = useState(() => typeof IntersectionObserver === 'undefined')
  const hostRef = useRef<HTMLDivElement | null>(null)
  const heightRef = useRef(height)
  // Whether the document in the frame has been themed and measured. Until then
  // the frame is hidden, so the message is never seen in its unthemed colors
  // at the placeholder height. That happens as soon as the document is parsed
  // (`readyWhenParsed` below), not at `load`, which waits for every picture.
  // A height remembered from an earlier mount sizes the frame but does not
  // show it: the document in it is a new one, and has not been themed.
  const [measured, setMeasured] = useState(false)
  // The document the frame was last themed and measured for, so a new one
  // starts over and one whose handler beat the effect below is left alone.
  const heightKeyRef = useRef<string | null>(null)
  const [frameDoc, setFrameDoc] = useState<Document | null>(null)
  // The ready handler is keyed on the document, not on the theme; the effect
  // below repaints a live frame when the theme changes under it.
  const appliedThemeRef = useRef<{ doc: Document; theme: typeof bubbleTheme } | null>(null)
  const bubbleThemeRef = useRef(bubbleTheme)
  bubbleThemeRef.current = bubbleTheme
  const [galleryItems, setGalleryItems] = useState<GalleryItem[]>([])
  const [galleryIndex, setGalleryIndex] = useState<number | null>(null)
  // Folds or unfolds the live document's quoted tail; null while it has none.
  const quoteKey = useMemo(() => bodyContentKey(html), [html])
  const foldQuoteRef = useRef<((open: boolean) => void) | null>(null)

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

  // A document measured before sizes its bubble before the first paint; the frame
  // only loads after that, so waiting for its measurement would show the bubble
  // at full width first.
  useLayoutEffect(() => {
    if (naturalWidths.has(documentKey)) onNaturalWidthRef.current?.(naturalWidths.get(documentKey) ?? null)
  }, [documentKey])

  // Only a new document starts over: hidden, at the height it measured to last
  // time or else the placeholder. A theme change keeps the height it has — the
  // frame is still showing a measured document — and the resize observer files
  // the new one if the canvas changes its box. The ready handler can run in the
  // same turn the iframe loads, before this effect; undoing its work would hide
  // the message it has just themed.
  useEffect(() => {
    if (heightKeyRef.current === documentKey) return
    const nextHeight = measuredHeights.get(`${documentKey}:${appearanceRef.current}`) ?? DEFAULT_FRAME_HEIGHT
    heightRef.current = nextHeight
    setHeight(nextHeight)
    setMeasured(false)
  }, [documentKey])

  useEffect(() => {
    const host = hostRef.current
    if (!host || typeof IntersectionObserver === 'undefined') {
      setNearViewport(true)
      return
    }

    const scrollRoot = host.closest('.message-scroll')
    const observer = new IntersectionObserver(
      ([entry]) => {
        const near = entry?.isIntersecting ?? false
        if (!near) setMeasured(false)
        setNearViewport(near)
      },
      {
        root: scrollRoot,
        rootMargin: FRAME_OVERSCAN,
      },
    )
    observer.observe(host)
    return () => observer.disconnect()
  }, [documentKey])

  const handleReady = useCallback(
    (doc: Document) => {
      // Not the document this handler was made for: about:blank on the first
      // wiring, or the previous one still in the frame. Its own handler is
      // still installed, and the load event wires this one again.
      if (doc.documentElement.getAttribute(FRAME_GENERATION_MARKER) !== generation) return

      // Before the first measurement, so a folded quote never flashes open.
      const foldQuote = installFrameQuoteFold(doc, quoteKey, {
        show: t('chat.showQuotedText'),
        hide: t('chat.hideQuotedText'),
      })
      foldQuoteRef.current = foldQuote

      let animationFrame = 0
      let disposed = false
      let themed = false
      // Per document: what its out-of-flow content was last seen to need.
      let overflowExtent = 0
      const cleanupFns: Array<() => void> = []
      cleanupFns.push(() => {
        if (foldQuoteRef.current === foldQuote) foldQuoteRef.current = null
      })

      const commitHeight = (nextHeight: number) => {
        if (disposed) return
        // Filed under the document this handler was installed for, in whatever
        // appearance is painting it now: a later document has its own handler.
        measuredHeights.set(`${documentKey}:${appearanceRef.current}`, nextHeight)
        heightKeyRef.current = documentKey
        setMeasured(true)
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

      // Only a document of plain flowing text can shrink its bubble: tables,
      // images and the like lay out against the width they are given. The body
      // is briefly laid out at max-content to read what its longest line needs;
      // it is restored before anything paints or any observer is delivered.
      const reportNaturalWidth = () => {
        const report = onNaturalWidthRef.current
        if (!report || !doc.body) return
        const commit = (width: number | null) => {
          naturalWidths.set(documentKey, width)
          report(width)
        }
        if (doc.querySelector(NON_TEXT_SELECTOR)) return commit(null)
        const style = doc.body.style
        // The base stylesheet caps the body at the frame's width (!important), which
        // would make this read no more than the current width: lift it while measuring.
        const saved = (prop: string) => [style.getPropertyValue(prop), style.getPropertyPriority(prop)] as const
        const [previous, previousPriority] = saved('width')
        const [previousMax, previousMaxPriority] = saved('max-width')
        style.setProperty('width', 'max-content', 'important')
        style.setProperty('max-width', 'none', 'important')
        const natural = Math.ceil(doc.body.getBoundingClientRect().width)
        if (previous) style.setProperty('width', previous, previousPriority)
        else style.removeProperty('width')
        if (previousMax) style.setProperty('max-width', previousMax, previousMaxPriority)
        else style.removeProperty('max-width')
        commit(natural > 0 ? natural : null)
      }

      const measure = () => {
        // Wired as soon as the document is parsed, which can be before the
        // frame has its column: a height read then is not the message's, and
        // would be shown and filed as if it were. The resize observer calls
        // again once there is a width.
        if (disposed || !themed || !doc.documentElement.clientWidth) return
        wrapOverflowingTables()
        reportNaturalWidth()
        const measurement = measureFrameHeight(frameMetrics(doc), overflowExtent)
        overflowExtent = measurement.overflowExtent
        commitHeight(measurement.height)
      }

      // The body can't scroll sideways (the frame is `scrolling="no"` so it can
      // self-size), so anything wider than the frame would be clipped outright.
      // Give the outermost overflowing table its own horizontal scroller — or,
      // with auto-fit on, shrink it to the bubble, the way the mobile reader
      // fits fixed-width mail. `zoom` rather than a transform: it scales the
      // layout box too, so the height measured below is the height drawn.
      const fitTables = new Map<HTMLTableElement, { natural: number; zoom: number }>()
      const wrapOverflowingTables = () => {
        const limit = doc.documentElement?.clientWidth ?? 0
        if (!limit) return
        for (const table of doc.querySelectorAll<HTMLTableElement>('table')) {
          if (table.closest('.meron-table-scroll')) {
            // An effect replay keeps the DOM wrappers but replaces this map.
            // Track only the wrapped table itself, not its nested tables.
            if (
              autoFitRef.current &&
              table.parentElement?.classList.contains('meron-table-scroll') &&
              !fitTables.has(table)
            ) {
              fitTables.set(table, {
                natural: Math.max(table.scrollWidth, table.offsetWidth),
                zoom: Number.parseFloat(table.style.getPropertyValue('zoom')) || 1,
              })
            }
            continue
          }
          const rect = table.getBoundingClientRect()
          const overflowsFrame = rect.left < -1 || rect.right > limit + 1
          const overflowsItself = table.scrollWidth > table.clientWidth + 1
          if (!overflowsFrame && !overflowsItself) continue

          const wrapper = doc.createElement('div')
          wrapper.className = 'meron-table-scroll'
          table.parentNode?.insertBefore(wrapper, table)
          wrapper.appendChild(table)
          // The width it lays out at unscaled, read once: re-reading it would
          // mean dropping the zoom on every measurement.
          if (autoFitRef.current) {
            fitTables.set(table, { natural: Math.max(table.scrollWidth, table.offsetWidth), zoom: 1 })
          }
        }
        for (const [table, fit] of fitTables) {
          const room = table.parentElement?.clientWidth ?? 0
          if (!room || !fit.natural) continue
          const zoom = Math.min(1, room / fit.natural)
          // Kept here, not read back off the style: the engine may serialise it
          // differently, and a write every measurement would relayout forever.
          if (Math.abs(zoom - fit.zoom) < 0.001) continue
          fit.zoom = zoom
          table.style.setProperty('zoom', String(zoom))
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
          copyText(pre.innerText).catch(() => undefined)
        })
        wrapper.appendChild(button)
      }

      // Observe media while the theme is prepared. Measurements wait for the
      // theme, and run in a separate frame after its work has yielded.
      const observer = new ResizeObserver(scheduleMeasure)
      observer.observe(doc.documentElement)
      if (doc.body) observer.observe(doc.body)

      for (const video of doc.querySelectorAll<HTMLVideoElement>('video[width][height]')) {
        reserveImageBox(video)
      }
      for (const image of doc.querySelectorAll<HTMLImageElement>('img')) {
        reserveImageBox(image)
        const onError = () => {
          // A blocked remote picture is an intentional placeholder, not a
          // failed attachment. Keep the sender's layout until it is revealed.
          const src = image.getAttribute('src') ?? ''
          if (allowRemote || !/^(?:https?:)?\/\//i.test(src)) releaseFailedImageBox(image)
          scheduleMeasure()
        }
        // Zero intrinsic width is valid for SVGs. Preserve those and intentional
        // remote placeholders; other completed zero-width requests have failed.
        const src = image.getAttribute('src') ?? ''
        if (image.complete && image.naturalWidth === 0 && src && !/\.svg(?:[?#]|$)|^data:image\/svg\+xml/i.test(src)) {
          onError()
        }
        const onLoad = () => {
          restoreImageBox(image)
          scheduleMeasure()
        }
        image.addEventListener('load', onLoad)
        image.addEventListener('error', onError)
        cleanupFns.push(() => {
          image.removeEventListener('load', onLoad)
          image.removeEventListener('error', onError)
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

      const initialTheme = bubbleThemeRef.current
      const remembersHeight = measuredHeights.has(`${documentKey}:${initialTheme.appearance}`)
      const finishTheme = () => {
        if (disposed) return
        themed = true
        appliedThemeRef.current = { doc, theme: initialTheme }
        setFrameDoc(doc)
        // A remount is already at the height this document measured to: show
        // it the moment it is themed, and let the measurement that follows
        // correct the height if the layout has changed since.
        if (remembersHeight) {
          heightKeyRef.current = documentKey
          setMeasured(true)
        }
        scheduleMeasure()
      }
      const themeFailed = (error: unknown) => {
        // A styling failure must not hide otherwise readable message text.
        if (disposed) return
        console.warn('Could not theme message HTML', error)
        try {
          applyBubbleTheme(doc, initialTheme)
        } catch (fallbackError) {
          console.warn('Could not apply fallback message theme', fallbackError)
        }
        finishTheme()
      }
      // Batched even for a remount: the walk over a large newsletter would
      // otherwise block scrolling once per frame that comes back into view.
      void applyBubbleThemeAsync(doc, initialTheme, () => disposed).then(finishTheme, themeFailed)
      cleanupFns.push(() => setFrameDoc((current) => (current === doc ? null : current)))

      return () => {
        disposed = true
        if (animationFrame) window.cancelAnimationFrame(animationFrame)
        observer.disconnect()
        window.clearTimeout(shortTimer)
        window.clearTimeout(longTimer)
        cleanupFns.forEach((cleanup) => cleanup())
      }
    },
    [documentKey, generation, quoteKey],
  )

  // Repaint a live frame when the theme changes under it: the document isn't
  // rebuilt for a theme (only typography is baked in), so nothing else would.
  useEffect(() => {
    if (!frameDoc?.body) return
    if (appliedThemeRef.current?.doc === frameDoc && appliedThemeRef.current.theme === bubbleTheme) return
    // A visible document changes palette atomically; only its initial hidden
    // theme walk yields, so intermediate canvas decisions cannot flash.
    applyBubbleTheme(frameDoc, bubbleTheme)
    appliedThemeRef.current = { doc: frameDoc, theme: bubbleTheme }
  }, [frameDoc, bubbleTheme])

  // A body can be shown before its attachment files are back on disk. When they
  // arrive the HTML is the same, so the frame keeps its document — and a picture
  // that already failed stays broken unless it is asked for again. Any drop in
  // the count is a file that came back, whether or not the rest followed.
  const mediaMissingRef = useRef(mediaMissing)
  const mediaRestoredRef = useRef(false)
  useEffect(() => {
    if (mediaMissing < mediaMissingRef.current) mediaRestoredRef.current = true
    mediaMissingRef.current = mediaMissing
    if (!mediaRestoredRef.current || !frameDoc) return
    // Pending pictures keep this recovery as a one-shot error retry.
    mediaRestoredRef.current = false
    reloadFailedImages(frameDoc)
  }, [mediaMissing, frameDoc])

  // Mark search hits in the live document. Re-runs when the query, the active
  // match, or the document itself changes; clearing on teardown keeps a frame
  // that outlives the search free of stale marks.
  useEffect(() => {
    if (!frameDoc) return
    const { activeMark } = applyFrameHighlights(frameDoc, searchQuery, activeSearchOffset)
    // A hit inside the folded quote has nothing to scroll to: unfold it first.
    if (activeMark && isInFoldedQuote(activeMark)) foldQuoteRef.current?.(true)
    // The frame doesn't scroll (it's sized to its content), so scrolling the
    // mark into view moves the conversation container around it — which is how
    // stepping between two hits inside one long message goes anywhere.
    activeMark?.scrollIntoView({ block: 'center', behavior: 'smooth' })
    return () => {
      // The document is gone once the frame reloads; ignore that case.
      if (frameDoc.defaultView) clearFrameHighlights(frameDoc)
    }
  }, [frameDoc, searchQuery, activeSearchOffset])

  return (
    <>
      <div ref={hostRef} style={{ height }} className="w-full">
        {nearViewport && (
          <HtmlFrame
            html={html}
            prepareHtml={prepareHtml}
            title={t('chat.messageHtml')}
            className="block w-full border-0 bg-transparent"
            style={{
              height,
              overflow: 'hidden',
              visibility: measured && heightKeyRef.current === documentKey ? 'visible' : 'hidden',
            }}
            scrolling="no"
            onFrameClick={handleFrameClick}
            onReady={handleReady}
            readyWhenParsed
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
