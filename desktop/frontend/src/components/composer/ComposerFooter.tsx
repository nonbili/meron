import { Paperclip, Image as ImageIcon, RefreshCw, Send, Type } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { sendShortcutLabel } from '../../states/settings'
import { IconButton } from '../button/IconButton'

type SaveStatus = 'idle' | 'saving' | 'saved' | 'error'

// One cloud for both save states, so only the mark inside it changes when a
// save finishes: lucide's cloud-upload, with its arrow swapped for a check
// mark of the same footprint (lucide's own cloud-check sits 2px lower).
function CloudStatusIcon({ saved, size }: { saved: boolean; size: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242" />
      {saved ? (
        <path d="m8.5 17 2.5 2.5 5-5.5" />
      ) : (
        <g className="animate-pulse">
          <path d="M12 13v8" />
          <path d="m8 17 4-4 4 4" />
        </g>
      )}
    </svg>
  )
}

// The composer's bottom action bar: attach/inline-image buttons, the rich/plain
// toggle, autosave status and the Discard/Send buttons.
export function ComposerFooter({
  rich,
  sending,
  saveStatus,
  saveError,
  canSend,
  onPickFiles,
  onPickInlineImages,
  onToggleRich,
  onDiscard,
  onSubmit,
}: {
  rich: boolean
  sending: boolean
  saveStatus: SaveStatus
  saveError?: string
  canSend: boolean
  onPickFiles: () => void
  onPickInlineImages: () => void
  onToggleRich: () => void
  onDiscard: () => void
  onSubmit: () => void
}) {
  const { t } = useTranslation()
  const draftAutosaveFailed = t('composer.status.draftAutosaveFailed')

  return (
    <div className="flex shrink-0 items-center justify-between gap-2 border-t border-border/60 bg-chat px-4 py-2.5 select-none">
      <div className="flex items-center gap-1">
        <IconButton
          icon={Paperclip}
          iconSize={16}
          label={t('composer.actions.attachFiles')}
          radius="xl"
          onClick={onPickFiles}
        />
        {rich && (
          <IconButton
            icon={ImageIcon}
            iconSize={16}
            label={t('composer.actions.insertInlineImage')}
            radius="xl"
            onClick={onPickInlineImages}
          />
        )}
        <button
          onClick={onToggleRich}
          className={`flex h-9 items-center gap-1.5 rounded-xl px-2.5 text-[0.6875rem] font-semibold transition-colors cursor-pointer ${
            rich ? 'bg-accent/10 text-accent' : 'text-secondary hover:bg-hover'
          }`}
          title={rich ? t('composer.actions.switchToPlainText') : t('composer.actions.switchToRichText')}
        >
          <Type size={15} />
          {rich ? t('composer.modes.richText') : t('composer.modes.plainText')}
        </button>
      </div>
      <div className="flex items-center gap-3">
        {/* Saving and saved are icons, not words: spelled out, the status
            wrapped onto three lines whenever the composer sat in a narrow pane. */}
        {saveStatus === 'saving' && (
          <span
            role="status"
            aria-label={t('composer.status.savingDraft')}
            title={t('composer.status.savingDraft')}
            className="flex items-center text-secondary"
          >
            <CloudStatusIcon saved={false} size={16} />
          </span>
        )}
        {saveStatus === 'saved' && (
          <span
            role="status"
            aria-label={t('composer.status.savedToServer')}
            title={t('composer.status.savedToServer')}
            className="flex items-center text-emerald-500"
          >
            <CloudStatusIcon saved size={16} />
          </span>
        )}
        {saveStatus === 'error' && (
          <span
            className="max-w-[360px] truncate text-[0.6875rem] text-rose-500 font-medium"
            title={saveError || draftAutosaveFailed}
          >
            {saveError || draftAutosaveFailed}
          </span>
        )}
        <button
          onClick={onDiscard}
          disabled={sending}
          className="rounded-xl px-4 py-2 text-xs font-semibold text-secondary transition-colors hover:bg-hover cursor-pointer disabled:opacity-50"
        >
          {t('buttons.discard')}
        </button>
        <button
          onClick={onSubmit}
          disabled={!canSend}
          title={t('composer.actions.sendWithShortcut', { shortcut: sendShortcutLabel('mod_enter') })}
          className={`flex items-center justify-center gap-1.5 rounded-xl px-5 py-2 text-xs font-bold transition-all ${
            !canSend
              ? 'cursor-not-allowed bg-hover text-secondary/70 shadow-none'
              : 'bg-accent text-accent-label shadow-md shadow-accent/15 hover:bg-accent-hover hover:text-accent-hover-label hover:shadow-lg hover:shadow-accent/20 active:scale-98 cursor-pointer'
          }`}
        >
          {sending ? <RefreshCw size={12} className="animate-spin" /> : <Send size={12} />}
          <span>{t('buttons.send')}</span>
        </button>
      </div>
    </div>
  )
}
