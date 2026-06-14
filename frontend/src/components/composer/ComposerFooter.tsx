import { Paperclip, Image as ImageIcon, RefreshCw, Send, Type } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { sendShortcutLabel } from '../../states/settings'
import { IconButton } from '../button/IconButton'

type SaveStatus = 'idle' | 'saving' | 'saved' | 'error'

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
    <div className="flex shrink-0 items-center justify-between gap-2 border-t border-border bg-header px-4 py-2.5 select-none">
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
          className={`flex h-9 items-center gap-1.5 rounded-xl px-2.5 text-[11px] font-semibold transition-colors cursor-pointer ${
            rich ? 'bg-accent/10 text-accent' : 'text-secondary hover:bg-hover'
          }`}
          title={rich ? t('composer.actions.switchToPlainText') : t('composer.actions.switchToRichText')}
        >
          <Type size={15} />
          {rich ? t('composer.modes.richText') : t('composer.modes.plainText')}
        </button>
      </div>
      <div className="flex items-center gap-3">
        {saveStatus === 'saving' && (
          <span className="flex items-center gap-1.5 text-[11px] text-secondary">
            <RefreshCw size={11} className="animate-spin" />
            <span>{t('composer.status.savingDraft')}</span>
          </span>
        )}
        {saveStatus === 'saved' && (
          <span className="flex items-center gap-1.5 text-[11px] text-emerald-500 font-medium">
            <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse" />
            <span>{t('composer.status.savedToServer')}</span>
          </span>
        )}
        {saveStatus === 'error' && (
          <span
            className="max-w-[360px] truncate text-[11px] text-rose-500 font-medium"
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
              : 'bg-accent text-white shadow-md shadow-accent/15 hover:bg-accent-hover hover:shadow-lg hover:shadow-accent/20 active:scale-98 cursor-pointer'
          }`}
        >
          {sending ? <RefreshCw size={12} className="animate-spin" /> : <Send size={12} />}
          <span>{t('buttons.send')}</span>
        </button>
      </div>
    </div>
  )
}
