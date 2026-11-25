import { useEffect, useId, useRef, useState } from 'react'
import type { Editor } from '@tiptap/react'
import { Link2, X } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { handleEditUndoKeyDown } from '../../lib/editUndo'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { Dialog } from '../dialog/Dialog'

export function LinkDialog({
  editor,
  onClose,
  portalContainer,
}: {
  editor: Editor
  onClose: () => void
  portalContainer?: HTMLElement
}) {
  const { t } = useTranslation()
  const id = useId()
  const input = useRef<HTMLInputElement>(null)
  const [selection] = useState(() => ({ from: editor.state.selection.from, to: editor.state.selection.to }))
  const [previous] = useState(() => (linkAtCaret(editor)?.attrs.href as string | undefined) ?? '')
  // Nothing selected and no link here: the URL goes in as its own text, as in
  // Gmail. Links don't grow as you type at their end, so a label can't be
  // typed into one afterwards; select the text to link it instead.
  const [insertsUrl] = useState(() => selection.from === selection.to && !linkAtCaret(editor))
  // Formatting picked for the next text (Bold at an empty caret), which
  // restoring the selection would drop.
  const [storedMarks] = useState(() => editor.state.storedMarks)
  const [url, setUrl] = useState(previous)

  useEffect(() => {
    input.current?.select()
  }, [])

  const apply = (href: string) => {
    if (!editor.isDestroyed) {
      const chain = editor.chain().focus().setTextSelection(selection)
      if (href && insertsUrl) {
        // Checked first, as setLink would: an address the link refuses must
        // not leave its text behind. Inserted as text, the URL keeps the
        // formatting around it.
        if (editor.can().setLink({ href })) {
          const end = selection.from + href.length
          chain
            .command(({ tr }) => {
              if (storedMarks) tr.setStoredMarks(storedMarks)
              return true
            })
            .insertContent({ type: 'text', text: href })
            .setTextSelection({ from: selection.from, to: end })
            .setLink({ href })
            .setTextSelection(end)
            .run()
        }
      } else if (href) chain.extendMarkRange('link').setLink({ href }).run()
      else chain.extendMarkRange('link').unsetLink().run()
    }
    onClose()
  }

  return (
    <Dialog
      onClose={onClose}
      initialFocusRef={input}
      portalContainer={portalContainer}
      aria-labelledby={`${id}-title`}
      className="max-w-sm p-6"
      onKeyDown={(event) => {
        handleEditUndoKeyDown(event.nativeEvent)
        // The shared dialog traps Tab at the document; composer shortcuts stay here.
        if (event.key !== 'Tab') event.stopPropagation()
      }}
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          event.stopPropagation()
          apply(url.trim())
        }}
      >
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-2xl bg-accent/10 text-accent">
              <Link2 size={17} />
            </div>
            <h2 id={`${id}-title`} className="text-[0.9375rem] font-bold tracking-tight">
              {t('composer.toolbar.link')}
            </h2>
          </div>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>
        <div className="flex flex-col gap-2">
          <label htmlFor={id} className="text-xs font-medium text-secondary">
            {t('composer.link.url')}
          </label>
          <input
            ref={input}
            id={id}
            value={url}
            onChange={(event) => setUrl(event.target.value)}
            placeholder="https://"
            inputMode="url"
            autoComplete="off"
            spellCheck={false}
            className="w-full rounded-xl border border-border bg-hover px-3.5 py-2.5 text-[0.8125rem] text-primary placeholder-secondary focus:outline-none focus:ring-1 focus:ring-accent"
          />
        </div>
        <div className="flex items-center justify-end gap-2">
          {previous && (
            <Button variant="ghost" className="mr-auto" onClick={() => apply('')}>
              {t('composer.link.remove')}
            </Button>
          )}
          <Button variant="secondary" onClick={onClose}>
            {t('buttons.cancel')}
          </Button>
          <Button type="submit" disabled={!url.trim() && !previous}>
            {t('buttons.save')}
          </Button>
        </div>
      </form>
    </Dialog>
  )
}

/** The link in the selection, or the one the caret is on, just after, or
 *  (at the start of a line) just before: links don't extend past their ends
 *  (see composerLink), so the editor no longer counts those spots as in one. */
export function linkAtCaret(editor: Editor) {
  const { $from, from, to, empty } = editor.state.selection
  const type = editor.schema.marks.link
  if (!type) return undefined
  if (!empty) {
    // The first link in the selection: its left edge reads the text before it.
    let found: ReturnType<typeof type.isInSet>
    editor.state.doc.nodesBetween(from, to, (node) => {
      found ??= type.isInSet(node.marks)
      return !found
    })
    return found
  }
  return (
    type.isInSet($from.marks()) ??
    type.isInSet($from.nodeBefore?.marks ?? []) ??
    // At the start of a line, the link it opens.
    ($from.nodeBefore ? undefined : type.isInSet($from.nodeAfter?.marks ?? []))
  )
}
