import { useEffect, useState, type MouseEvent } from 'react'
import { ChevronLeft, Copy, Download, File, Image, Loader2, Play, SquarePen, Users, X } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { openExternal } from '../../lib/native'
import { downloadAttachment } from '../../states/mail'
import { Avatar } from '../avatar/Avatar'
import { IconButton } from '../button/IconButton'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import { fileIconFor, formatFileSize } from './messageHelpers'

export type ConversationMediaItem = {
  type: 'image' | 'video'
  src: string
  filename: string
  // External/source URL, used to open videos the in-app webview can't decode.
  url?: string
  // For images, the index into the thread-wide gallery list so a thumbnail can
  // open the same lightbox the message bubbles use.
  galleryIndex?: number
}

export type ConversationFileItem = {
  filename: string
  size: number
  // Media key for the on-disk bytes; null when the file wasn't persisted.
  key: string | null
}

export type Participant = {
  name: string
  email: string
  /** How many of the thread's messages this address appears on (From/To/Cc). */
  count: number
  /** True when the address belongs to one of the user's own accounts. */
  isSelf: boolean
}

interface ConversationDetailsPanelProps {
  media: ConversationMediaItem[]
  files: ConversationFileItem[]
  participants: Participant[]
  scopeTitle: string
  scopeSubtitle?: string
  /** True while a freshly selected thread is still loading; the lists below
   * belong to the previous thread until it resolves. */
  loading: boolean
  onOpenImage: (galleryIndex: number) => void
  onComposeTo: (person: Participant) => void
  onClose: () => void
}

type View = 'overview' | 'media' | 'files'

export function ConversationDetailsPanel({
  media,
  files,
  participants,
  scopeTitle,
  scopeSubtitle,
  loading,
  onOpenImage,
  onComposeTo,
  onClose,
}: ConversationDetailsPanelProps) {
  const { t } = useTranslation()
  const [view, setView] = useState<View>('overview')
  const [personMenu, setPersonMenu] = useState<{ x: number; y: number; person: Participant } | null>(null)

  useEffect(() => {
    setView('overview')
  }, [scopeTitle])

  // Close the overlay drawer on Escape (matches the gallery/search behavior).
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [onClose])

  return (
    <>
      {/* Backdrop — only below the push-aside breakpoint, where the panel is an overlay. */}
      <div className="min-[1100px]:hidden absolute inset-0 z-20 bg-black/40 animate-fade-in" onClick={onClose} />

      <aside
        className="
          absolute inset-y-0 right-0 z-30 w-[340px] max-w-[85vw] shadow-2xl
          min-[1100px]:relative min-[1100px]:inset-auto min-[1100px]:z-auto min-[1100px]:shadow-none min-[1100px]:max-w-none
          flex shrink-0 flex-col border-l border-border bg-header
          animate-slide-in-right
        "
      >
        {/* Header */}
        {view === 'overview' ? (
          <div className="flex h-12 shrink-0 items-center justify-end border-b border-border px-4">
            <IconButton icon={X} iconSize={18} label={t('chat.closeEsc')} onClick={onClose} />
          </div>
        ) : (
          <div className="flex min-h-16 shrink-0 items-center justify-between gap-3 border-b border-border px-4 py-3">
            <div className="min-w-0">
              <h3 className="truncate text-sm font-bold text-primary">
                {view === 'media' ? t('chat.media') : t('chat.files')}
              </h3>
              <p className="mt-0.5 truncate text-[11px] font-medium text-secondary" title={scopeTitle}>
                {scopeTitle}
              </p>
              {scopeSubtitle && (
                <p className="truncate text-[10px] text-secondary/80" title={scopeSubtitle}>
                  {scopeSubtitle}
                </p>
              )}
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <IconButton
                icon={ChevronLeft}
                iconSize={18}
                label={t('buttons.back')}
                onClick={() => setView('overview')}
              />
              <IconButton icon={X} iconSize={18} label={t('chat.closeEsc')} onClick={onClose} />
            </div>
          </div>
        )}

        {/* Content */}
        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <div className="flex h-full items-center justify-center">
              <Loader2 size={20} className="animate-spin text-secondary" />
            </div>
          ) : view === 'overview' ? (
            <Overview
              mediaCount={media.length}
              filesCount={files.length}
              participants={participants}
              onOpenMedia={() => setView('media')}
              onOpenFiles={() => setView('files')}
              onComposeTo={onComposeTo}
              onOpenPersonMenu={setPersonMenu}
            />
          ) : view === 'media' ? (
            media.length === 0 ? (
              <EmptyHint text={t('chat.noMedia')} />
            ) : (
              <div className="grid grid-cols-3 gap-1.5 p-3">
                {media.map((item, idx) => (
                  <button
                    key={idx}
                    type="button"
                    onClick={() => {
                      if (item.type === 'image' && item.galleryIndex !== undefined) {
                        onOpenImage(item.galleryIndex)
                      } else if (item.url) {
                        openExternal(item.url)
                      }
                    }}
                    className="group relative aspect-square overflow-hidden rounded-md border border-border/30 bg-black/5 dark:bg-white/5 hover:opacity-90 cursor-pointer"
                    title={item.filename}
                  >
                    {item.type === 'image' ? (
                      <img src={item.src} alt={item.filename} loading="lazy" className="h-full w-full object-cover" />
                    ) : (
                      // Static tile — never a live <video>. Each <video> spins up a
                      // GStreamer pipeline in WebKitGTK; a grid of them freezes the
                      // webview. Click opens the source in the system player instead.
                      <span className="flex h-full w-full items-center justify-center bg-black/75 text-white/90">
                        <Play size={22} fill="currentColor" />
                      </span>
                    )}
                  </button>
                ))}
              </div>
            )
          ) : files.length === 0 ? (
            <EmptyHint text={t('chat.noFiles')} />
          ) : (
            <div className="flex flex-col gap-1.5 p-3">
              {files.map((file, idx) => {
                const downloadable = !!file.key
                const FileIcon = fileIconFor(file.filename, file.mime)
                return (
                  <button
                    key={idx}
                    type="button"
                    disabled={!downloadable}
                    onClick={() => downloadAttachment(file)}
                    title={downloadable ? t('chat.saveFile', { filename: file.filename }) : file.filename}
                    className={`group flex items-center gap-2.5 rounded-xl border border-border/30 bg-black/[0.02] dark:bg-white/[0.02] p-2.5 text-left ${
                      downloadable
                        ? 'hover:bg-black/[0.05] dark:hover:bg-white/[0.05] cursor-pointer'
                        : 'cursor-default'
                    }`}
                  >
                    <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent/10 text-accent">
                      <FileIcon size={16} />
                    </span>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-xs font-semibold text-primary">{file.filename}</p>
                      <p className="text-[10px] text-secondary">{formatFileSize(file.size)}</p>
                    </div>
                    {downloadable && (
                      <Download
                        size={15}
                        className="shrink-0 text-secondary opacity-0 group-hover:opacity-100 transition-opacity"
                      />
                    )}
                  </button>
                )
              })}
            </div>
          )}
        </div>
      </aside>
      {personMenu && (
        <FloatingContextMenu
          x={personMenu.x}
          y={personMenu.y}
          onClose={() => setPersonMenu(null)}
          overlay
          overlayClassName="fixed inset-0 z-[80]"
          className="fixed z-[81] min-w-[180px] rounded-xl border border-border bg-header p-1 shadow-xl"
          onContextMenu={(event) => event.preventDefault()}
        >
          <MenuItem
            icon={<Copy size={13} className="text-accent" />}
            label={t('chat.copyEmailAddress')}
            className="whitespace-nowrap"
            onClick={() => {
              navigator.clipboard?.writeText(personMenu.person.email).catch(() => undefined)
              setPersonMenu(null)
            }}
          />
        </FloatingContextMenu>
      )}
    </>
  )
}

function Overview({
  mediaCount,
  filesCount,
  participants,
  onOpenMedia,
  onOpenFiles,
  onComposeTo,
  onOpenPersonMenu,
}: {
  mediaCount: number
  filesCount: number
  participants: Participant[]
  onOpenMedia: () => void
  onOpenFiles: () => void
  onComposeTo: (person: Participant) => void
  onOpenPersonMenu: (menu: { x: number; y: number; person: Participant }) => void
}) {
  const { t } = useTranslation()
  return (
    <div>
      {(mediaCount > 0 || filesCount > 0) && (
        <div className="border-b border-border bg-chats p-3">
          {mediaCount > 0 && (
            <SummaryRow icon={Image} label={t('chat.mediaItems', { count: mediaCount })} onClick={onOpenMedia} />
          )}
          {filesCount > 0 && (
            <SummaryRow icon={File} label={t('chat.fileItems', { count: filesCount })} onClick={onOpenFiles} />
          )}
        </div>
      )}
      <div className="p-3">
        <div className="mb-2 flex items-center gap-2 px-2 text-[11px] font-bold uppercase tracking-wide text-secondary">
          <Users size={15} />
          <span>{t('chat.people', { count: participants.length })}</span>
        </div>
        <PeopleList participants={participants} onComposeTo={onComposeTo} onOpenPersonMenu={onOpenPersonMenu} />
      </div>
    </div>
  )
}

function SummaryRow({ icon: Icon, label, onClick }: { icon: LucideIcon; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full cursor-pointer items-center gap-3 rounded-lg px-2 py-3 text-left text-sm font-medium text-primary hover:bg-black/[0.04] dark:hover:bg-white/[0.04]"
    >
      <Icon size={21} className="shrink-0" />
      <span>{label}</span>
    </button>
  )
}

function PeopleList({
  participants,
  onComposeTo,
  onOpenPersonMenu,
}: {
  participants: Participant[]
  onComposeTo: (person: Participant) => void
  onOpenPersonMenu: (menu: { x: number; y: number; person: Participant }) => void
}) {
  const { t } = useTranslation()
  if (participants.length === 0) {
    return <EmptyHint text={t('chat.noParticipants')} />
  }

  const openMenu = (event: MouseEvent, person: Participant) => {
    event.preventDefault()
    onOpenPersonMenu({ x: event.clientX, y: event.clientY, person })
  }

  return (
    <div className="flex flex-col gap-0.5">
      {participants.map((person) => (
        <div
          key={person.email}
          onMouseDown={(event) => {
            if (event.button === 2) openMenu(event, person)
          }}
          onContextMenu={(event) => openMenu(event, person)}
          className="group flex items-center gap-3 rounded-xl p-2 text-left hover:bg-black/[0.04] dark:hover:bg-white/[0.04]"
        >
          <Avatar name={person.name} email={person.email} size={36} />
          <div className="min-w-0 flex-1">
            <p className="flex items-center gap-1.5 truncate text-xs font-semibold text-primary selectable-text">
              <span className="truncate">{person.name}</span>
              {person.isSelf && (
                <span className="shrink-0 rounded-full bg-accent/15 px-1.5 py-px text-[9px] font-bold uppercase tracking-wide text-accent">
                  {t('chat.you')}
                </span>
              )}
            </p>
            <p className="truncate text-[11px] text-secondary selectable-text">{person.email}</p>
          </div>
          <button
            type="button"
            onClick={() => onComposeTo(person)}
            title={t('chat.newMessageTo', { email: person.email })}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-secondary opacity-0 transition-all hover:bg-black/[0.06] hover:text-primary group-hover:opacity-100 dark:hover:bg-white/[0.08] cursor-pointer"
          >
            <SquarePen size={15} />
          </button>
        </div>
      ))}
    </div>
  )
}

function EmptyHint({ text }: { text: string }) {
  return <div className="flex h-full items-center justify-center px-6 text-center text-xs text-secondary">{text}</div>
}
