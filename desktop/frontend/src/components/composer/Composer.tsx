import { useRef } from 'react'
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
import { DropOverlay } from '../chat/DropOverlay'
import { routeComposerDrop, useFileDrop } from '../../lib/useFileDrop'

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
    addFiles,
    handlePaste,
    handleKeyDown,
    setLink,
    submit,
  } = useComposer(tabId)

  // Two drop areas while files are dragged in: the message body, where images go
  // inline (rich text only), and the rest of the composer, which attaches them.
  // Files that can't be shown inline are attached wherever they land. A drag
  // that may be a link (Linux) lands on the body and the fields themselves.
  const bodyRef = useRef<HTMLDivElement | null>(null)
  const {
    dragging: fileDragging,
    mayHoldImages,
    mayBeLink,
    dropHandlers,
  } = useFileDrop(
    (files, { handledByChild, zone }) => {
      const { inline, attach } = routeComposerDrop(files, {
        inBody: zone === 'body',
        rich: !!draft?.rich,
        handledByChild,
      })
      if (inline.length > 0) addFiles(inline, { inline: true, afterSelection: true })
      if (attach.length > 0) addFiles(attach)
    },
    { zoneOf: (target) => (target instanceof Node && bodyRef.current?.contains(target) ? 'body' : 'composer') },
  )

  if (!draft) return null

  const showInlineDrop = fileDragging && draft.rich && mayHoldImages

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
    <div
      className="relative flex flex-1 flex-col overflow-hidden bg-chat"
      onKeyDown={handleSendShortcut}
      {...dropHandlers}
    >
      <ComposerHeaderFields
        draft={draft}
        update={update}
        focusTo={!focusBody}
        onFocusBody={() => {
          if (draft.rich) {
            if (!editor || editor.isDestroyed) return false
            editor.view.focus()
            return editor.view.hasFocus()
          }
          const body = textRef.current
          body?.focus()
          return !!body && body.ownerDocument.activeElement === body
        }}
      />

      {draft.rich && editor && <ComposerToolbar editor={editor} onSetLink={setLink} />}

      {/* Body */}
      <div ref={bodyRef} className="relative flex min-h-0 flex-1 flex-col">
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
              className="h-full min-h-[240px] w-full resize-none bg-transparent text-[calc(0.875rem*var(--me-message-scale))] leading-relaxed text-primary placeholder-secondary outline-none"
            />
          )}
        </div>
        {showInlineDrop && <DropOverlay kind="inline" className="inset-2" passThrough={mayBeLink} />}
      </div>

      {showInlineDrop && (
        <div className="relative mx-2 mb-1 h-16 shrink-0">
          <DropOverlay className="inset-0" />
        </div>
      )}

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
      {fileDragging && !showInlineDrop && <DropOverlay passThrough={mayBeLink} />}
    </div>
  )
}
