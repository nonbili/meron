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
import { ComposerQuote } from './ComposerQuote'
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
    expandQuote,
    removeQuote,
    pickAttachmentFiles,
    pickInlineImages,
    addFiles,
    handlePaste,
    handleKeyDown,
    setLink,
    submit,
    closeKeepingDraft,
  } = useComposer(tabId)

  // Two drop areas while files are dragged in: the message body, where images go
  // inline (rich text only), and the rest of the composer, which attaches them.
  // Files that can't be shown inline are attached wherever they land. The body's
  // overlay is only drawn: the drop lands on the editor, which places images
  // where they were dropped (useComposer's handleDrop). A drag that may be a
  // link (Linux) lands on the fields themselves too.
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
    {
      zoneOf: (target) => (target instanceof Node && bodyRef.current?.contains(target) ? 'body' : 'composer'),
      scope: tabId,
    },
  )

  if (!draft) return null

  const showInlineDrop = fileDragging && draft.rich && mayHoldImages

  // In the full editor, bare Enter must always insert a newline (long-form /
  // rich body), so send is bound to Cmd/Ctrl+Enter regardless of the global
  // send-shortcut setting. Bound on the outer container so it fires from any
  // field (To/Subject/body).
  //
  // Esc closes the tab and keeps the message in Drafts, from any field too.
  // Menus, suggestion lists and dialogs inside take their Esc first
  // (useEscapeKey stops it before it gets here), and an IME keeps the one
  // that ends its composition, which WebKit reports as keyCode 229 instead of
  // isComposing. defaultPrevented is no sign of either: ProseMirror sets it on
  // every Esc in the rich-text body.
  const handleSendShortcut = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !e.shiftKey) {
      e.preventDefault()
      void submit()
      return
    }
    if (e.nativeEvent.isComposing || e.keyCode === 229) return
    if (e.key === 'Escape' && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
      e.preventDefault()
      // Not on to the app-wide Esc, which would close the conversation pane.
      e.stopPropagation()
      void closeKeepingDraft()
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
        {/* Without a quote the plain-text textarea fills this box and scrolls
            itself: it is a block (an inline one leaves a baseline gap that made
            this box scroll too) and holds the side padding, so its scrollbar
            sits at the pane edge.
            With a folded quote the body only grows with what is typed, so the
            quote's dots sit right under the text rather than below an empty
            box; clicking the space left below still writes into the body. */}
        <div
          className={`flex-1 overflow-y-auto py-3${draft.quote || draft.rich ? ' px-4' : ''}${draft.quote ? ' [&_.tiptap-body]:min-h-[4.5em]' : ''}`}
          onKeyDown={handleKeyDown}
          onMouseDown={(event) => {
            if (!draft.quote || event.target !== event.currentTarget) return
            event.preventDefault()
            if (draft.rich) {
              editor?.commands.focus('end')
            } else {
              const body = textRef.current
              body?.focus()
              body?.setSelectionRange(body.value.length, body.value.length)
            }
          }}
        >
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
              rows={draft.quote ? Math.max(3, draft.text.split('\n').length + 1) : undefined}
              className={`${draft.quote ? 'min-h-[4.5em]' : 'block h-full min-h-[240px] px-4'} w-full resize-none bg-transparent text-[calc(0.875rem*var(--me-message-scale))] leading-relaxed text-primary placeholder-secondary outline-none`}
            />
          )}
          {draft.quote && <ComposerQuote onExpand={expandQuote} onRemove={removeQuote} />}
        </div>
        {showInlineDrop && <DropOverlay kind="inline" className="inset-2" passThrough />}
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
