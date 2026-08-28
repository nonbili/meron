import { EditorContent } from '@tiptap/react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { settings$ } from '../../states/settings'
import { useComposer } from './useComposer'
import { ComposerHeaderFields } from './ComposerHeaderFields'
import { ComposerToolbar } from './ComposerToolbar'
import { ComposerAttachments } from './ComposerAttachments'
import { ComposerFooter } from './ComposerFooter'
import { closeMessageTab } from '../../states/compose'

export function Composer({ tabId }: { tabId: string }) {
  const { t } = useTranslation()
  const spellCheck = useValue(settings$.spellCheck)
  const {
    draft,
    editor,
    focusBody,
    textRef,
    sending,
    error,
    saveStatus,
    saveError,
    canSend,
    update,
    toggleRich,
    pickAttachmentFiles,
    pickInlineImages,
    handlePaste,
    handleKeyDown,
    setLink,
    submit,
  } = useComposer(tabId)

  if (!draft) return null

  // In the full editor, bare Enter must always insert a newline (long-form /
  // rich body), so send is bound to Cmd/Ctrl+Enter regardless of the global
  // send-shortcut setting. Bound on the outer container so it fires from any
  // field (To/Subject/body).
  const handleSendShortcut = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !e.shiftKey) {
      e.preventDefault()
      void submit()
    }
  }

  return (
    <div className="flex flex-1 flex-col overflow-hidden bg-chat" onKeyDown={handleSendShortcut}>
      <ComposerHeaderFields draft={draft} update={update} focusTo={!focusBody} />

      {draft.rich && editor && <ComposerToolbar editor={editor} onSetLink={setLink} />}

      {/* Body */}
      <div className="flex-1 overflow-y-auto px-4 py-3" onKeyDown={handleKeyDown}>
        {draft.rich ? (
          <EditorContent editor={editor} />
        ) : (
          <textarea
            ref={textRef}
            autoFocus={focusBody}
            value={draft.text}
            onChange={(e) => update({ text: e.target.value })}
            onPaste={handlePaste}
            placeholder={t('composer.placeholders.message')}
            spellCheck={spellCheck}
            className="h-full min-h-[240px] w-full resize-none bg-transparent text-[0.875rem] leading-relaxed text-primary placeholder-secondary outline-none"
          />
        )}
      </div>

      <ComposerAttachments
        attachments={draft.attachments}
        onRemove={(id) => update({ attachments: draft.attachments.filter((a) => a.id !== id) })}
      />

      {error && <p className="shrink-0 px-4 pb-1 text-[0.6875rem] font-medium text-rose-500">{error}</p>}

      <ComposerFooter
        rich={draft.rich}
        sending={sending}
        saveStatus={error ? 'idle' : saveStatus}
        saveError={saveError}
        canSend={canSend}
        onPickFiles={() => void pickAttachmentFiles()}
        onPickInlineImages={() => void pickInlineImages()}
        onToggleRich={toggleRich}
        onDiscard={() => void closeMessageTab(tabId)}
        onSubmit={submit}
      />
    </div>
  )
}
