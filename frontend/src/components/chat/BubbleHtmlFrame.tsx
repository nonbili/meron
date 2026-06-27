import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from '../../lib/i18n'
import { Gallery, type GalleryItem } from './Gallery'
import { HtmlFrame } from './HtmlFrame'
import { prepareBubbleHtml } from './bubbleHtml'

const MIN_FRAME_HEIGHT = 80
const DEFAULT_FRAME_HEIGHT = 120
const HEIGHT_CHANGE_EPSILON = 1
const measuredHeights = new Map<string, number>()

function cacheKeyForHtml(html: string) {
  let hash = 0
  for (let index = 0; index < html.length; index += 1) {
    hash = (Math.imul(hash, 31) + html.charCodeAt(index)) | 0
  }
  return `${html.length}:${hash}`
}

function clampHeight(height: number) {
  return Math.max(MIN_FRAME_HEIGHT, Math.ceil(height))
}

// Renders an email's HTML body in a self-sizing sandboxed iframe, wraps each
// <pre> in a copy-code affordance and tracks the content height so the frame
// grows to fit while the bubble wrapper owns scrolling.
export function BubbleHtmlFrame({ html }: { html: string }) {
  const { t } = useTranslation()
  const cacheKey = useMemo(() => cacheKeyForHtml(html), [html])
  const cachedHeight = measuredHeights.get(cacheKey)
  const [height, setHeight] = useState(() => cachedHeight ?? DEFAULT_FRAME_HEIGHT)
  const [measured, setMeasured] = useState(() => cachedHeight !== undefined)
  const heightRef = useRef(height)
  const measuredRef = useRef(measured)
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

  const handleReady = useCallback(
    (doc: Document) => {
      let animationFrame = 0
      let disposed = false
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
        const bodyRect = doc.body?.getBoundingClientRect()
        const bodyHeight = bodyRect ? bodyRect.top + bodyRect.height : 0
        const nextHeight = clampHeight(
          Math.max(
            bodyHeight,
            doc.body?.scrollHeight ?? 0,
            doc.documentElement?.scrollHeight ?? 0,
            doc.body?.offsetHeight ?? 0,
            doc.documentElement?.offsetHeight ?? 0,
          ),
        )
        commitHeight(nextHeight)
      }

      for (const pre of doc.querySelectorAll<HTMLPreElement>('pre')) {
        if (pre.closest('.meron-code-block')) continue

        const wrapper = doc.createElement('div')
        wrapper.className = 'meron-code-block'
        pre.parentNode?.insertBefore(wrapper, pre)
        wrapper.appendChild(pre)

        const button = doc.createElement('button')
        button.type = 'button'
        button.className = 'meron-copy-code'
        button.title = 'Copy code'
        button.setAttribute('aria-label', 'Copy code')
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

  return (
    <>
      <HtmlFrame
        html={html}
        prepareHtml={prepareBubbleHtml}
        title={t('chat.messageHtml')}
        className="block w-full border-0 bg-transparent"
        style={{ height, overflow: 'hidden', visibility: measured ? 'visible' : 'hidden' }}
        scrolling="no"
        onFrameClick={handleFrameClick}
        onReady={handleReady}
        forwardContextMenu
      />
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
