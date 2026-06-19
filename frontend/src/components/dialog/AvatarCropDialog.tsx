import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Image as ImageIcon, Minus, Plus, RotateCcw, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { showToast } from '../../states/ui'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'

const CROP_SIZE = 240
const OUTPUT_SIZE = 512

type ImageSize = { width: number; height: number }
type Offset = { x: number; y: number }

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max)
}

function coverScale(size: ImageSize) {
  return Math.max(CROP_SIZE / size.width, CROP_SIZE / size.height)
}

function clampOffset(offset: Offset, zoom: number, size: ImageSize): Offset {
  const scale = coverScale(size) * zoom
  const maxX = Math.max(0, (size.width * scale - CROP_SIZE) / 2)
  const maxY = Math.max(0, (size.height * scale - CROP_SIZE) / 2)
  return {
    x: clamp(offset.x, -maxX, maxX),
    y: clamp(offset.y, -maxY, maxY),
  }
}

function canvasToPngFile(canvas: HTMLCanvasElement, fileName: string): Promise<File> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (!blob) {
        reject(new Error('Could not render cropped avatar'))
        return
      }
      const baseName = fileName.replace(/\.[^.]+$/, '') || 'avatar'
      resolve(new File([blob], `${baseName}-avatar.png`, { type: 'image/png' }))
    }, 'image/png')
  })
}

export function AvatarCropDialog({
  file,
  busy,
  onCancel,
  onSave,
}: {
  file: File
  busy?: boolean
  onCancel: () => void
  onSave: (file: File) => Promise<void>
}) {
  const { t } = useTranslation()
  const [imageUrl, setImageUrl] = useState('')
  const [imageSize, setImageSize] = useState<ImageSize | null>(null)
  const [zoom, setZoom] = useState(1)
  const [offset, setOffset] = useState<Offset>({ x: 0, y: 0 })
  const [saving, setSaving] = useState(false)
  const imageRef = useRef<HTMLImageElement | null>(null)
  const dragRef = useRef<{ pointerId: number; startX: number; startY: number; startOffset: Offset } | null>(null)

  useEffect(() => {
    const url = URL.createObjectURL(file)
    setImageUrl(url)
    setImageSize(null)
    setZoom(1)
    setOffset({ x: 0, y: 0 })
    return () => URL.revokeObjectURL(url)
  }, [file])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busy && !saving) {
        event.stopPropagation()
        onCancel()
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [busy, onCancel, saving])

  const rendered = useMemo(() => {
    if (!imageSize) return null
    const scale = coverScale(imageSize) * zoom
    return {
      width: imageSize.width * scale,
      height: imageSize.height * scale,
    }
  }, [imageSize, zoom])

  const setClampedZoom = (nextZoom: number) => {
    if (!imageSize) return
    const normalized = clamp(nextZoom, 1, 4)
    setZoom(normalized)
    setOffset((current) => clampOffset(current, normalized, imageSize))
  }

  const nudgeZoom = (delta: number) => setClampedZoom(zoom + delta)

  const resetCrop = () => {
    setZoom(1)
    setOffset({ x: 0, y: 0 })
  }

  const onImageLoad = () => {
    const image = imageRef.current
    if (!image) return
    setImageSize({ width: image.naturalWidth, height: image.naturalHeight })
    setOffset({ x: 0, y: 0 })
    setZoom(1)
  }

  const startDrag = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!imageSize || busy || saving) return
    event.currentTarget.setPointerCapture(event.pointerId)
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      startOffset: offset,
    }
  }

  const moveDrag = (event: React.PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current
    if (!drag || !imageSize || drag.pointerId !== event.pointerId) return
    const nextOffset = {
      x: drag.startOffset.x + event.clientX - drag.startX,
      y: drag.startOffset.y + event.clientY - drag.startY,
    }
    setOffset(clampOffset(nextOffset, zoom, imageSize))
  }

  const endDrag = (event: React.PointerEvent<HTMLDivElement>) => {
    if (dragRef.current?.pointerId === event.pointerId) {
      dragRef.current = null
    }
  }

  const saveCrop = useCallback(async () => {
    const image = imageRef.current
    if (!image || !imageSize) return
    setSaving(true)
    try {
      const canvas = document.createElement('canvas')
      canvas.width = OUTPUT_SIZE
      canvas.height = OUTPUT_SIZE
      const ctx = canvas.getContext('2d')
      if (!ctx) throw new Error(t('avatar.renderFailed'))

      const scale = coverScale(imageSize) * zoom
      const sourceSize = CROP_SIZE / scale
      const centerX = imageSize.width / 2 - offset.x / scale
      const centerY = imageSize.height / 2 - offset.y / scale
      const sourceX = clamp(centerX - sourceSize / 2, 0, imageSize.width - sourceSize)
      const sourceY = clamp(centerY - sourceSize / 2, 0, imageSize.height - sourceSize)

      ctx.imageSmoothingQuality = 'high'
      ctx.drawImage(image, sourceX, sourceY, sourceSize, sourceSize, 0, 0, OUTPUT_SIZE, OUTPUT_SIZE)

      const croppedFile = await canvasToPngFile(canvas, file.name)
      await onSave(croppedFile)
    } catch (error) {
      showToast(error instanceof Error ? error.message : t('avatar.cropFailed'), 'error')
    } finally {
      setSaving(false)
    }
  }, [file.name, imageSize, offset.x, offset.y, onSave, zoom])

  const disabled = busy || saving || !imageSize

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/45 dark:bg-black/65 backdrop-blur-[3px] p-4">
      <div className="w-full max-w-sm rounded-3xl border border-border bg-chats text-primary shadow-2xl animate-slide-up overflow-hidden">
        <div className="flex items-center justify-between gap-3 border-b border-border/70 px-5 py-4">
          <div className="min-w-0">
            <h3 className="text-[14px] font-bold leading-tight">{t('avatar.edit')}</h3>
            <p className="mt-0.5 truncate text-[10.5px] font-medium text-secondary">{file.name}</p>
          </div>
          <IconButton
            icon={X}
            iconSize={15}
            label={t('buttons.cancel')}
            size="sm"
            onClick={onCancel}
            disabled={busy || saving}
          />
        </div>

        <div className="flex flex-col items-center gap-4 px-5 py-5">
          <div
            className="relative touch-none overflow-hidden rounded-3xl bg-app shadow-inner ring-1 ring-border"
            style={{ width: CROP_SIZE, height: CROP_SIZE }}
            onPointerDown={startDrag}
            onPointerMove={moveDrag}
            onPointerUp={endDrag}
            onPointerCancel={endDrag}
          >
            {!imageSize && (
              <div className="absolute inset-0 flex items-center justify-center text-secondary">
                <ImageIcon size={22} />
              </div>
            )}
            {imageUrl && (
              <img
                ref={imageRef}
                src={imageUrl}
                alt=""
                draggable={false}
                onLoad={onImageLoad}
                onError={() => showToast(t('avatar.loadFailed'), 'error')}
                className="absolute left-1/2 top-1/2 max-w-none select-none"
                style={{
                  width: rendered?.width,
                  height: rendered?.height,
                  transform: `translate(-50%, -50%) translate(${offset.x}px, ${offset.y}px)`,
                }}
              />
            )}
            <div className="pointer-events-none absolute inset-0 rounded-3xl ring-2 ring-white/90 dark:ring-white/70" />
          </div>

          <div className="flex w-full items-center gap-3">
            <IconButton
              icon={Minus}
              iconSize={14}
              label={t('avatar.zoomOut')}
              size="sm"
              onClick={() => nudgeZoom(-0.1)}
              disabled={disabled || zoom <= 1}
            />
            <input
              type="range"
              min={1}
              max={4}
              step={0.01}
              value={zoom}
              onChange={(event) => setClampedZoom(Number(event.target.value))}
              disabled={disabled}
              aria-label={t('avatar.zoom')}
              className="app-range flex-1"
            />
            <IconButton
              icon={Plus}
              iconSize={14}
              label={t('avatar.zoomIn')}
              size="sm"
              onClick={() => nudgeZoom(0.1)}
              disabled={disabled || zoom >= 4}
            />
            <IconButton
              icon={RotateCcw}
              iconSize={14}
              label={t('avatar.resetCrop')}
              size="sm"
              onClick={resetCrop}
              disabled={disabled}
            />
          </div>
        </div>

        <div className="flex justify-end gap-2 border-t border-border/70 px-5 py-4">
          <Button variant="secondary" size="sm" onClick={onCancel} disabled={busy || saving}>
            {t('buttons.cancel')}
          </Button>
          <Button size="sm" onClick={() => void saveCrop()} disabled={disabled}>
            {t('avatar.save')}
          </Button>
        </div>
      </div>
    </div>
  )
}
