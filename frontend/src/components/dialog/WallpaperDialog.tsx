import { useState, useEffect } from 'react'
import { X, Check, Upload, Image as ImageIcon } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import type { ChatWallpaper } from '../../types'
import { WALLPAPER_PRESETS, sanitizeChatWallpaper, wallpaperCss } from '../../lib/wallpapers'
import { pickImageFile } from '../../lib/nativeFilePicker'
import { showToast } from '../../states/ui'
import { IconButton } from '../button/IconButton'

function wallpaperKey(wallpaper: ChatWallpaper | null) {
  if (!wallpaper) return 'preset:plain'
  return wallpaper.kind === 'preset' ? `preset:${wallpaper.presetId}` : `custom:${wallpaper.url}`
}

// Wallpaper picker shared by account chat backgrounds and kanban board
// backgrounds: the owner decides where the selection persists via callbacks.
export function WallpaperDialog({
  title,
  previewName,
  wallpaper: rawWallpaper,
  onSelect,
  onUploadFile,
  onClose,
}: {
  title: string
  /** Seeds the avatar initial in the live preview mockup. */
  previewName?: string
  wallpaper: ChatWallpaper | null | undefined
  onSelect: (wallpaper: ChatWallpaper | null) => void | Promise<void>
  /** Upload a custom image and persist it as the selection; should throw on failure. */
  onUploadFile: (file: File) => Promise<void>
  onClose: () => void
}) {
  const { t } = useTranslation()
  const [busy, setBusy] = useState(false)
  const wallpaper = sanitizeChatWallpaper(rawWallpaper)
  const selectedKey = wallpaperKey(wallpaper)

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation()
        onClose()
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [onClose])

  const uploadWallpaper = async () => {
    try {
      const file = await pickImageFile(t('wallpaper.chooseImage'))
      if (!file) return
      setBusy(true)
      await onUploadFile(file)
    } catch (error) {
      showToast(error instanceof Error ? error.message : t('wallpaper.uploadFailed'), 'error')
    } finally {
      setBusy(false)
    }
  }

  const previewInfo = wallpaperCss(wallpaper)

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/45 dark:bg-black/65 backdrop-blur-[3px] p-4 animate-fade-in">
      <div className="w-full max-w-4xl h-[620px] max-h-[90vh] rounded-3xl border border-border bg-chats text-primary shadow-2xl animate-slide-up flex flex-col overflow-hidden">
        {/* Title Header */}
        <div className="flex items-center justify-between gap-3 border-b border-border/70 px-6 py-4 shrink-0">
          <div className="flex items-center gap-2">
            <ImageIcon className="text-accent" size={16} />
            <h3 className="text-[14px] font-bold leading-tight">{title}</h3>
          </div>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>

        {/* Split Panel Body */}
        <div className="flex flex-1 min-h-0 overflow-hidden flex-col md:flex-row">
          {/* Left panel: Wallpapers grid */}
          <div className="flex-1 overflow-y-auto p-6 flex flex-col gap-4">
            <div className="grid grid-cols-2 sm:grid-cols-3 gap-3 pb-2">
              {/* Custom Upload Card */}
              <button
                type="button"
                onClick={() => void uploadWallpaper()}
                disabled={busy}
                className={`relative flex aspect-[16/10] flex-col items-center justify-center gap-1.5 overflow-hidden rounded-xl border border-dashed cursor-pointer transition-all ${
                  selectedKey.startsWith('custom:')
                    ? 'border-accent text-accent bg-accent/5 ring-2 ring-accent/20'
                    : 'border-border text-secondary hover:text-accent hover:border-accent/50 hover:bg-accent/2'
                } disabled:opacity-50`}
              >
                {wallpaper?.kind === 'custom' && (
                  <span
                    className="absolute inset-0 bg-cover bg-center"
                    style={{
                      backgroundImage: `linear-gradient(rgba(15, 23, 42, 0.15), rgba(15, 23, 42, 0.15)), url("${wallpaper.url}")`,
                    }}
                  />
                )}
                <span className="relative flex flex-col items-center gap-1 rounded-lg bg-chats/90 px-3 py-2 border border-border/30 shadow-xs">
                  <Upload size={15} />
                  <span className="text-[10px] font-bold leading-none">
                    {busy ? t('wallpaper.uploading') : t('wallpaper.uploadCustom')}
                  </span>
                </span>
                {selectedKey.startsWith('custom:') && (
                  <span className="absolute top-2 right-2 flex h-5 w-5 items-center justify-center rounded-full bg-accent text-white shadow-xs">
                    <Check size={11} />
                  </span>
                )}
              </button>

              {/* Preset Cards */}
              {WALLPAPER_PRESETS.map((preset) => {
                const selected = selectedKey === `preset:${preset.id}`
                return (
                  <button
                    key={preset.id}
                    type="button"
                    title={preset.name}
                    onClick={() => void onSelect({ kind: 'preset', presetId: preset.id })}
                    className={`relative aspect-[16/10] overflow-hidden rounded-xl border cursor-pointer transition-all ${
                      selected
                        ? 'border-accent ring-2 ring-accent/20'
                        : 'border-border hover:border-secondary/40 hover:scale-[1.01]'
                    }`}
                  >
                    <span className={`absolute inset-0 ${preset.previewClass}`} />
                    {selected && (
                      <span className="absolute top-2 right-2 flex h-5 w-5 items-center justify-center rounded-full bg-accent text-white shadow-xs">
                        <Check size={11} />
                      </span>
                    )}
                  </button>
                )
              })}
            </div>
          </div>

          {/* Right panel: Live Mockup Chat Preview */}
          <div className="w-full md:w-[320px] shrink-0 border-t md:border-t-0 md:border-l border-border/70 bg-raised p-5 flex flex-col select-none">
            {/* Chat screen mockup frame */}
            <div className="flex-1 rounded-2xl border border-border overflow-hidden flex flex-col bg-chat relative shadow-inner min-h-[280px]">
              {/* Wallpaper background inside mockup */}
              <div
                className={`absolute inset-0 transition-all duration-300 ${previewInfo.className}`}
                style={previewInfo.style}
              />

              {/* Overlay to dim mockup and ensure contrast */}
              <div className="absolute inset-0 bg-gradient-to-b from-black/5 to-transparent pointer-events-none" />

              {/* Chat bubbles container */}
              <div className="relative z-10 flex-1 flex flex-col justify-end p-3.5 gap-3">
                <div className="mx-auto select-none rounded-full bg-active border border-border/30 px-2.5 py-0.8 text-center text-[9px] font-bold text-secondary/80">
                  Today
                </div>

                {/* Left Bubble (Incoming) */}
                <div className="flex items-end gap-1.5 max-w-[85%] self-start">
                  <div className="h-5 w-5 rounded-full bg-accent/80 flex items-center justify-center text-[8.5px] font-bold text-white shadow-xs">
                    {previewName ? previewName.slice(0, 1) : 'U'}
                  </div>
                  <div className="rounded-2xl rounded-bl-sm border border-border bg-chats p-2.5 text-[11px] leading-normal text-primary shadow-xs">
                    How does this chat wallpaper look on your screen?
                  </div>
                </div>

                {/* Right Bubble (Outgoing) */}
                <div className="flex flex-col max-w-[80%] self-end">
                  <div className="rounded-2xl rounded-br-sm border border-accent/20 bg-accent text-white p-2.5 text-[11px] leading-normal shadow-xs">
                    Looks fantastic! The text contrast and background pattern are perfectly balanced.
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
