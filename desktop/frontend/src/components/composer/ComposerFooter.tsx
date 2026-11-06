import { Paperclip, Image as ImageIcon, RefreshCw, Send, Type } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { sendShortcutLabel } from '../../states/settings'
import { IconButton } from '../button/IconButton'
import type { RecipientKeys } from '../../lib/crypto'
import { EncryptToggle } from '../crypto/EncryptToggle'

type SaveStatus = 'idle' | 'saving' | 'saved' | 'error'

// One cloud for both save states, so only the mark inside it changes when a
// save finishes: lucide's cloud-upload, with its arrow swapped for a check
// mark of the same footprint (lucide's own cloud-check sits 2px lower).
// Nudged down 1px: centred by its box, the cloud read high next to the
// lowercase of "Discard" (measured on Linux, 2x).
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
      className="translate-y-px"
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
  encryption,
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
  /** The end-to-end encryption toggle, when the account can encrypt. */
  encryption?: {
    keys: RecipientKeys | null
    on: boolean
    sensitive?: 'reply' | 'forward'
    hasRecipients?: boolean
    onChange: (on: boolean) => void
  }
  onPickFiles: () => void
  onPickInlineImages: () => void
  onToggleRich: () => void
  onDiscard: () => void
  onSubmit: () => void
}) {
  const { t } = useTranslation()
  const draftAutosaveFailed = t('composer.status.draftAutosaveFailed')
  const saved = saveStatus === 'saved'
  const saveStatusLabel = saved
    ? t('composer.status.savedToServer')
    : saveStatus === 'saving'
      ? t('composer.status.savingDraft')
      : ''

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
        {encryption && (
          <EncryptToggle
            size="md"
            keys={encryption.keys}
            on={encryption.on}
            sensitive={encryption.sensitive}
            hasRecipients={encryption.hasRecipients}
            onChange={encryption.onChange}
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
        {/* Always mounted, with only its text changing: screen readers announce
            changes inside a live region, not a region that appears already filled. */}
        <span role="status" className="sr-only">
          {saveStatusLabel}
        </span>
        {saveStatusLabel && (
          <span
            title={saveStatusLabel}
            className={`flex items-center ${saved ? 'text-emerald-500' : 'text-secondary'}`}
          >
            <CloudStatusIcon saved={saved} size={16} />
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
