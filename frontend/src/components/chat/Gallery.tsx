import { useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { ChevronLeft, ChevronRight, Copy, Download, X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { copyAttachmentImage, downloadAttachment } from '../../states/mail'
import { MenuItem } from '../menu/MenuItem'
import { VideoAttachment } from './VideoAttachment'

// The native WebKitGTK image context menu ("Save Image As", "Open Image in New
// Window") is dead inside the Wails webview — there's no browser download or
// navigation backing it. Derive the media key from a `/media/<key>` src so we
// can save through the bridge instead.
function mediaKeyFromSrc(src: string): string | null {
  const prefix = '/media/'
  return src.startsWith(prefix) ? decodeURIComponent(src.slice(prefix.length)) : null
}

const MIN_SCALE = 1
const MAX_SCALE = 8
const ZOOM_SPEED = 0.0025

export type GalleryItem = {
  // Defaults to 'image' so callers that only ever build image lists (e.g. the
  // HTML-frame galleries) don't need to tag their items.
  type?: 'image' | 'video'
  src: string
  filename: string
  // External/source URL for videos, used by the system-player fallback.
  url?: string
  // The text of the message the media came from, shown as a one-line caption.
  caption?: string
}

interface GalleryProps {
  items: GalleryItem[]
  index: number
  onIndexChange: (index: number) => void
  onClose: () => void
}

export function Gallery({ items, index, onIndexChange, onClose }: GalleryProps) {
  const { t } = useTranslation()
  const total = items.length
  const hasPrev = index > 0
  const hasNext = index < total - 1

  const stageRef = useRef<HTMLDivElement>(null)
  const [scale, setScale] = useState(1)
  const [offset, setOffset] = useState({ x: 0, y: 0 })
  const [isPanning, setIsPanning] = useState(false)
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null)
  const zoomed = scale > 1

  // Pan gesture state. `moved` lets us tell a pan-drag from a click so a
  // drag that ends on the backdrop doesn't dismiss the gallery.
  const pan = useRef<{
    active: boolean
    startX: number
    startY: number
    originX: number
    originY: number
    moved: boolean
  } | null>(null)

  const resetZoom = useCallback(() => {
    setScale(1)
    setOffset({ x: 0, y: 0 })
  }, [])

  const goPrev = useCallback(() => {
    if (index > 0) onIndexChange(index - 1)
  }, [index, onIndexChange])

  const goNext = useCallback(() => {
    if (index < total - 1) onIndexChange(index + 1)
  }, [index, total, onIndexChange])

  // Reset zoom and dismiss any open menu whenever we page to a different image.
  useEffect(() => {
    resetZoom()
    setMenu(null)
  }, [index, resetZoom])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
      else if (event.key === 'ArrowLeft') goPrev()
      else if (event.key === 'ArrowRight') goNext()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [onClose, goPrev, goNext])

  // Ctrl + wheel (and trackpad pinch, which browsers report as ctrl+wheel)
  // zooms toward the cursor. Attached natively so we can preventDefault the
  // page scroll — React's onWheel is passive.
  useEffect(() => {
    const stage = stageRef.current
    if (!stage) return

    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey) return
      event.preventDefault()

      const rect = stage.getBoundingClientRect()
      const cx = event.clientX - (rect.left + rect.width / 2)
      const cy = event.clientY - (rect.top + rect.height / 2)

      setScale((prevScale) => {
        const next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, prevScale * Math.exp(-event.deltaY * ZOOM_SPEED)))
        if (next === prevScale) return prevScale
        if (next === MIN_SCALE) {
          setOffset({ x: 0, y: 0 })
          return next
        }
        // Keep the content point under the cursor fixed while scaling.
        const ratio = next / prevScale
        setOffset((prev) => ({
          x: cx - (cx - prev.x) * ratio,
          y: cy - (cy - prev.y) * ratio,
        }))
        return next
      })
    }

    stage.addEventListener('wheel', onWheel, { passive: false })
    return () => stage.removeEventListener('wheel', onWheel)
  }, [])

  const onPointerDown = (event: React.PointerEvent) => {
    if (!zoomed) return
    event.stopPropagation()
    ;(event.target as Element).setPointerCapture(event.pointerId)
    setIsPanning(true)
    pan.current = {
      active: true,
      startX: event.clientX,
      startY: event.clientY,
      originX: offset.x,
      originY: offset.y,
      moved: false,
    }
  }

  const onPointerMove = (event: React.PointerEvent) => {
    const p = pan.current
    if (!p?.active) return
    const dx = event.clientX - p.startX
    const dy = event.clientY - p.startY
    if (Math.abs(dx) > 3 || Math.abs(dy) > 3) p.moved = true
    setOffset({ x: p.originX + dx, y: p.originY + dy })
  }

  const onPointerUp = (event: React.PointerEvent) => {
    if (pan.current?.active) {
      pan.current.active = false
      setIsPanning(false)
      event.stopPropagation()
    }
  }

  const current = items[index]
  if (!current) return null

  const saveCurrent = () => {
    setMenu(null)
    const key = mediaKeyFromSrc(current.src)
    if (!key) return
    void downloadAttachment({ key, filename: current.filename })
  }
  const copyCurrent = () => {
    setMenu(null)
    const key = mediaKeyFromSrc(current.src)
    if (!key) return
    void copyAttachmentImage({ key })
  }
  const canSave = mediaKeyFromSrc(current.src) !== null

  // Portal to <body> so the fixed overlay isn't trapped inside a transformed or
  // overflow-clipped ancestor (e.g. a chat bubble's slide-up animation), which
  // would otherwise mis-position the overlay and clip its controls.
  return createPortal(
    <div
      className="fixed inset-0 z-50 flex flex-col bg-black/90 backdrop-blur-sm animate-fade-in select-none"
      onClick={onClose}
    >
      {/* Top bar: counter + close */}
      <div className="flex shrink-0 items-center justify-between px-4 py-3 text-white/90">
        <span className="text-xs font-semibold tabular-nums">{total > 1 ? `${index + 1} / ${total}` : ''}</span>
        <button
          onClick={(event) => {
            event.stopPropagation()
            onClose()
          }}
          className="flex h-9 w-9 items-center justify-center rounded-full hover:bg-white/15 cursor-pointer transition-colors"
          title={t('chat.closeEsc')}
        >
          <X size={20} />
        </button>
      </div>

      {/* Image stage */}
      <div ref={stageRef} className="relative flex flex-1 items-center justify-center overflow-hidden px-4 pb-4">
        {hasPrev && (
          <button
            onClick={(event) => {
              event.stopPropagation()
              goPrev()
            }}
            className="absolute left-3 z-10 flex h-11 w-11 items-center justify-center rounded-full bg-black/40 text-white/90 hover:bg-black/70 cursor-pointer transition-colors"
            title={t('chat.previousImage')}
          >
            <ChevronLeft size={26} />
          </button>
        )}

        {current.type === 'video' ? (
          // Keyed by index so paging remounts the player: the poster comes back
          // and the previous clip's pipeline is torn down by the unmount effect.
          // Zoom/pan/copy stay image-only; the native controls own the gestures.
          <div
            key={index}
            onClick={(event) => event.stopPropagation()}
            className="flex w-full max-w-4xl items-center justify-center"
          >
            <VideoAttachment
              src={current.src}
              externalUrl={current.url ?? current.src}
              externalLabel={t('chat.openExternalPlayer')}
              className="group relative w-full"
              videoClassName="w-full max-h-[78vh] rounded-md bg-black shadow-2xl"
              posterClassName="flex aspect-video w-full items-center justify-center rounded-md bg-black text-white/90 transition-colors hover:text-white cursor-pointer shadow-2xl"
            />
          </div>
        ) : (
          <img
            src={current.src}
            alt={current.filename}
            draggable={false}
            onClick={(event) => event.stopPropagation()}
            onContextMenu={(event) => {
              if (!canSave) return // let the default menu show for remote URLs
              event.preventDefault()
              event.stopPropagation()
              setMenu({ x: event.clientX, y: event.clientY })
            }}
            onDoubleClick={(event) => {
              event.stopPropagation()
              resetZoom()
            }}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            style={{
              transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
              cursor: zoomed ? (isPanning ? 'grabbing' : 'grab') : 'default',
              transition: isPanning ? 'none' : 'transform 0.12s ease-out',
            }}
            className="max-h-full max-w-full object-contain rounded-md shadow-2xl will-change-transform"
          />
        )}

        {hasNext && (
          <button
            onClick={(event) => {
              event.stopPropagation()
              goNext()
            }}
            className="absolute right-3 z-10 flex h-11 w-11 items-center justify-center rounded-full bg-black/40 text-white/90 hover:bg-black/70 cursor-pointer transition-colors"
            title={t('chat.nextImage')}
          >
            <ChevronRight size={26} />
          </button>
        )}
      </div>

      {menu && (
        <>
          <div
            className="fixed inset-0 z-[60]"
            onClick={(event) => {
              event.stopPropagation()
              setMenu(null)
            }}
            onContextMenu={(event) => {
              event.preventDefault()
              event.stopPropagation()
              setMenu(null)
            }}
          />
          <div
            className="fixed z-[61] min-w-[160px] rounded-xl border border-border bg-header p-1 shadow-xl"
            style={{ top: menu.y, left: menu.x }}
            onClick={(event) => event.stopPropagation()}
          >
            <MenuItem
              icon={<Download size={13} className="text-accent" />}
              label={t('chat.actions.saveImage')}
              onClick={saveCurrent}
            />
            <MenuItem
              icon={<Copy size={13} className="text-accent" />}
              label={t('chat.actions.copyImage')}
              onClick={copyCurrent}
            />
          </div>
        </>
      )}

      {/* Caption: message text, falling back to the filename */}
      {(current.caption || current.filename) && (
        <div
          className="shrink-0 px-6 pb-4 text-center text-xs text-white/70 truncate"
          title={current.caption || current.filename}
        >
          {current.caption || current.filename}
        </div>
      )}
    </div>,
    document.body,
  )
}
