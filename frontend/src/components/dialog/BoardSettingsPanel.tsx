import { useEffect, useState } from 'react'
import { Camera, Columns3, Image as ImageIcon, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { KanbanBoard } from '../../states/settings'
import {
  removeKanbanBoard,
  renameKanbanBoard,
  setKanbanBoardAvatar,
  setKanbanBoardWallpaper,
} from '../../states/kanban'
import { confirmAction, showToast, ui$ } from '../../states/ui'
import { WALLPAPER_PRESETS, sanitizeChatWallpaper, wallpaperCss } from '../../lib/wallpapers'
import { readFileData } from '../../lib/readFileData'
import { pickImageFile } from '../../lib/nativeFilePicker'
import { writeBoardAvatarFile, writeBoardWallpaperFile } from '../../lib/boardMedia'
import { AvatarCropDialog } from './AvatarCropDialog'
import { WallpaperDialog } from './WallpaperDialog'
import { SettingRow, SettingsGroup, TextRow } from './AccountSettingsRows'

function BoardTile({ board, size, className = '' }: { board: KanbanBoard; size: number; className?: string }) {
  if (board.avatarUrl) {
    return (
      <img
        src={board.avatarUrl}
        alt=""
        style={{ width: size, height: size }}
        className={`shrink-0 rounded-2xl object-cover ring-1 ring-black/5 dark:ring-white/10 ${className}`}
      />
    )
  }
  return (
    <div
      style={{ width: size, height: size }}
      className={`flex shrink-0 items-center justify-center rounded-2xl bg-accent/10 text-accent border border-accent/10 ${className}`}
    >
      <Columns3 size={Math.round(size * 0.45)} />
    </div>
  )
}

// Board image upload with the same square-crop flow accounts use. Owns the
// pending-file state; the crop dialog renders while a file is selected.
function useBoardImage(boardId: string) {
  const { t } = useTranslation()
  const [pendingFile, setPendingFile] = useState<File | null>(null)
  const [busy, setBusy] = useState(false)

  const pickFile = async () => {
    try {
      setPendingFile(await pickImageFile(t('kanban.board.chooseImage')))
    } catch (error) {
      showToast(error instanceof Error ? error.message : t('kanban.board.chooseImageFailed'), 'error')
    }
  }

  const saveCropped = async (file: File) => {
    setBusy(true)
    try {
      const data = await readFileData(file)
      const url = await writeBoardAvatarFile(boardId, { name: file.name, mime: file.type, data })
      setKanbanBoardAvatar(boardId, url)
      setPendingFile(null)
    } catch (error) {
      showToast(error instanceof Error ? error.message : t('kanban.board.uploadImageFailed'), 'error')
    } finally {
      setBusy(false)
    }
  }

  return { pendingFile, setPendingFile, busy, pickFile, saveCropped }
}

function BoardNameRow({ board }: { board: KanbanBoard }) {
  const { t } = useTranslation()
  const [name, setName] = useState(board.name)

  useEffect(() => {
    setName(board.name)
  }, [board.id, board.name])

  return (
    <TextRow
      title={t('kanban.board.name')}
      value={name}
      placeholder={t('kanban.board.namePlaceholder')}
      onChange={(value) => {
        setName(value)
        renameKanbanBoard(board.id, value)
      }}
    />
  )
}

function BoardWallpaperCard({ board }: { board: KanbanBoard }) {
  const { t } = useTranslation()
  const [dialogOpen, setDialogOpen] = useState(false)
  const wallpaper = sanitizeChatWallpaper(board.wallpaper)

  const presetName =
    wallpaper?.kind === 'preset'
      ? (WALLPAPER_PRESETS.find((p) => p.id === wallpaper.presetId)?.name ?? t('wallpaper.plain'))
      : wallpaper?.kind === 'custom'
        ? t('wallpaper.customBackground')
        : t('wallpaper.plainDefault')

  const previewInfo = wallpaperCss(wallpaper)

  return (
    <SettingRow
      icon={<ImageIcon size={15} />}
      title={t('kanban.board.background')}
      control={
        <div className="flex items-center gap-3 select-none">
          <span className="text-[11px] font-semibold text-secondary truncate max-w-32">{presetName}</span>
          <div
            className={`h-7 w-11 rounded-lg border border-border/80 overflow-hidden relative shadow-inner shrink-0 ${previewInfo.className}`}
            style={previewInfo.style}
          />
          <button
            type="button"
            onClick={() => setDialogOpen(true)}
            className="rounded-xl bg-active border border-border/30 px-3 py-1.5 text-xs font-semibold text-primary hover:bg-active hover:border-border/50 cursor-pointer transition-colors"
          >
            {t('common.change')}
          </button>
          {dialogOpen && (
            <WallpaperDialog
              title={t('wallpaper.boardTitle', { name: board.name })}
              previewName={board.name}
              wallpaper={board.wallpaper ?? null}
              onSelect={(next) => setKanbanBoardWallpaper(board.id, next)}
              onUploadFile={async (file) => {
                const data = await readFileData(file)
                const url = await writeBoardWallpaperFile(board.id, { name: file.name, mime: file.type, data })
                setKanbanBoardWallpaper(board.id, { kind: 'custom', url })
              }}
              onClose={() => setDialogOpen(false)}
            />
          )}
        </div>
      }
    />
  )
}

// Settings panel for one kanban board: image, name, and background, mirroring
// the account panel layout.
export function BoardPanel({ board }: { board: KanbanBoard }) {
  const { t } = useTranslation()
  const image = useBoardImage(board.id)
  const columnCount = board.columns.length

  const onDelete = async () => {
    if (
      await confirmAction({
        title: t('kanban.board.deleteTitle'),
        message: t('kanban.board.deleteMessage', { name: board.name }),
        confirmLabel: t('buttons.delete'),
        tone: 'danger',
      })
    ) {
      ui$.accountSettingsId.set('')
      removeKanbanBoard(board.id)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-3 min-w-0">
        <button
          type="button"
          title={t('kanban.board.changeImage')}
          disabled={image.busy}
          onClick={() => void image.pickFile()}
          className="relative shrink-0 rounded-2xl group disabled:cursor-default cursor-pointer"
        >
          <BoardTile board={board} size={40} />
          <span className="absolute inset-0 flex items-center justify-center rounded-2xl bg-black/45 opacity-0 group-hover:opacity-100 transition-opacity">
            <Camera size={15} className="text-white" />
          </span>
        </button>
        {image.pendingFile && (
          <AvatarCropDialog
            file={image.pendingFile}
            busy={image.busy}
            onCancel={() => image.setPendingFile(null)}
            onSave={image.saveCropped}
          />
        )}
        <div className="min-w-0 flex-1">
          <h2 className="text-[15px] font-bold tracking-tight leading-tight truncate">{board.name}</h2>
          <p className="text-[10.5px] text-secondary mt-0.5 font-medium truncate">
            {t('kanban.board.subtitle', { count: columnCount })}
          </p>
        </div>
      </div>

      <SettingsGroup title={t('kanban.board.label')}>
        <BoardNameRow board={board} />
        <BoardWallpaperCard board={board} />
      </SettingsGroup>

      <button
        type="button"
        onClick={() => void onDelete()}
        className="mt-1 self-start flex items-center gap-1.5 rounded-lg px-2 py-1 text-xs font-semibold text-secondary hover:text-rose-500 transition-colors cursor-pointer"
      >
        <Trash2 size={12} />
        {t('kanban.board.delete')}
      </button>
    </div>
  )
}
