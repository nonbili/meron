import { useEffect, useState, type ReactNode } from 'react'
import { useValue } from '@legendapp/state/react'
import Markdown from 'react-markdown'
import { Pencil, Plus, StickyNote } from 'lucide-react'
import { clsx } from '../../lib/utils'
import { MarkdownIcon } from '../icons/Icons'
import { contactKey, contactNotes$, loadContactNote, saveContactNote } from '../../states/contacts'
import { useTranslation } from '../../lib/i18n'
import { openExternal } from '../../lib/native'
import { Button } from '../button/Button'

/** One place a contact is kept (an address book, or this device): a card
 * with where it is in the heading and what is kept there under it. */
export function ContactSection({
  icon,
  title,
  subtitle,
  actions,
  children,
}: {
  icon: ReactNode
  title: string
  subtitle?: string
  actions?: ReactNode
  children: ReactNode
}) {
  return (
    <section className="mb-2 rounded-xl border border-border/60 px-3 pt-2 pb-3" aria-label={title}>
      <div className="flex min-h-9 items-center gap-2.5">
        <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-hover text-secondary">
          {icon}
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-sm font-semibold" title={title}>
            {title}
          </h2>
          {subtitle && (
            <p className="truncate text-xs text-secondary" title={subtitle}>
              {subtitle}
            </p>
          )}
        </div>
        {actions}
      </div>
      <div className="mt-2">{children}</div>
    </section>
  )
}

export const noteEditButtonClass =
  'inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-[0.6875rem] font-semibold text-secondary transition-colors hover:bg-hover hover:text-primary'

/** A dashed "Add a note…" button, for a contact with no note yet. */
export function AddNoteButton({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation()
  return (
    <button
      className="flex w-full items-center gap-2 rounded-lg border border-dashed border-border px-3 py-2.5 text-left text-sm text-secondary transition-colors hover:border-accent/50 hover:text-accent"
      onClick={onClick}
    >
      <Plus size={14} className="stroke-[2.5]" />
      {t('contacts.notesPlaceholder')}
    </button>
  )
}

/** The note kept only on this device for `addr`, edited in place. */
export function ContactNotes({
  addr,
  title,
  subtitle,
  icon,
  footer,
  autoEdit = false,
  onClose,
}: {
  addr: string
  /** Defaults to "Notes". */
  title?: string
  subtitle?: string
  /** Shown before the title: where this note is kept. */
  icon?: ReactNode
  /** Shown under the note while it is not being edited. */
  footer?: ReactNode
  /** Open with the editor showing. */
  autoEdit?: boolean
  /** After the editor closes, saved or cancelled. */
  onClose?: () => void
}) {
  const { t } = useTranslation()
  const key = contactKey(addr)
  const note = useValue(contactNotes$[key])
  const [editing, setEditing] = useState(autoEdit)
  const [preview, setPreview] = useState(false)
  useEffect(() => {
    void loadContactNote(key)
  }, [key])
  const dirty = !!note && note.draft !== note.saved
  const text = note?.draft ?? ''
  const editorOpen = editing || dirty
  const showPreview = preview && !!text
  const startEditing = () => {
    setEditing(true)
    setPreview(false)
  }
  const cancel = () => {
    const current = contactNotes$[key].peek()
    contactNotes$[key].draft.set(current.saved)
    contactNotes$[key].error.set(false)
    setEditing(false)
    setPreview(false)
    onClose?.()
  }
  const save = async () => {
    await saveContactNote(key)
    const current = contactNotes$[key].peek()
    if (!current.error && current.draft === current.saved) {
      setEditing(false)
      setPreview(false)
      onClose?.()
    }
  }
  const rendered = <NoteMarkdown text={text} />
  const tab = (active: boolean) =>
    clsx(
      'rounded-md px-2 py-1 text-[0.6875rem] font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-40',
      active ? 'bg-hover text-primary' : 'text-secondary hover:text-primary',
    )
  return (
    <ContactSection
      icon={icon ?? <StickyNote size={14} aria-hidden />}
      title={title ?? t('contacts.notes')}
      subtitle={subtitle}
      actions={
        note?.loaded &&
        !editorOpen &&
        text && (
          <button className={noteEditButtonClass} onClick={startEditing}>
            <Pencil size={12} className="stroke-[2.5]" />
            {t('contacts.notesEdit')}
          </button>
        )
      }
    >
      {!note?.loaded ? (
        note?.error ? (
          <button className="text-xs text-accent" onClick={() => void loadContactNote(key)}>
            {t('contacts.retry')}
          </button>
        ) : (
          <p className="text-xs text-secondary">{t('common.loading')}</p>
        )
      ) : editorOpen ? (
        <div className="overflow-hidden rounded-lg border border-border/70 transition-shadow focus-within:border-accent/50 focus-within:ring-2 focus-within:ring-accent/10">
          <div className="flex items-center gap-0.5 border-b border-border/50 px-1.5 py-1">
            <button className={tab(!showPreview)} aria-pressed={!showPreview} onClick={() => setPreview(false)}>
              {t('contacts.notesWrite')}
            </button>
            <button
              className={tab(showPreview)}
              aria-pressed={showPreview}
              disabled={!text}
              onClick={() => setPreview(true)}
            >
              {t('contacts.notesPreview')}
            </button>
            <span
              className="ml-auto px-1.5 text-secondary/70"
              role="img"
              aria-label={t('contacts.markdownHint')}
              title={t('contacts.markdownHint')}
            >
              <MarkdownIcon size={16} />
            </span>
          </div>
          {showPreview ? (
            <div className="min-h-28 px-3 py-2.5">{rendered}</div>
          ) : (
            <textarea
              autoFocus
              className="block min-h-28 w-full resize-y bg-transparent px-3 py-2.5 text-sm leading-relaxed outline-none placeholder:text-secondary/70"
              aria-label={t('contacts.notes')}
              placeholder={t('contacts.notesPlaceholder')}
              value={text}
              onChange={(event) => contactNotes$[key].draft.set(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Escape' && !note.saving) {
                  // Handled here so the panel's own Escape handling doesn't also fire.
                  event.preventDefault()
                  event.stopPropagation()
                  cancel()
                } else if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
                  event.preventDefault()
                  if (!note.saving && dirty) void save()
                }
              }}
            />
          )}
          <div className="flex items-center justify-end gap-1 border-t border-border/50 bg-hover/30 p-1.5">
            <Button size="sm" variant="ghost" disabled={note.saving} onClick={cancel}>
              {t('buttons.cancel')}
            </Button>
            <Button size="sm" disabled={note.saving || !dirty} onClick={() => void save()}>
              {t(note.saving ? 'contacts.notesSaving' : 'contacts.notesSave')}
            </Button>
          </div>
        </div>
      ) : text ? (
        rendered
      ) : (
        <AddNoteButton onClick={startEditing} />
      )}
      {/* Not tied to the note: what it offers must not wait on it loading. */}
      {!editorOpen && footer}
      {note?.loaded && note.error && (
        <p role="status" className="mt-2 text-xs text-red-500">
          {t('contacts.notesSaveError')}
        </p>
      )}
    </ContactSection>
  )
}

/** A note as Markdown, safe to show: no remote images, links opened natively. */
export function NoteMarkdown({ text }: { text: string }) {
  return (
    <div className="break-words text-sm leading-relaxed text-primary [&>:first-child]:mt-0 [&>:last-child]:mb-0 [&_p]:my-2 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_h1]:mt-3 [&_h1]:mb-1.5 [&_h1]:text-base [&_h1]:font-semibold [&_h2]:mt-3 [&_h2]:mb-1.5 [&_h2]:text-sm [&_h2]:font-semibold [&_h3]:mt-2 [&_h3]:mb-1 [&_h3]:font-semibold [&_pre]:my-2 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:bg-hover [&_pre]:p-2 [&_code]:rounded [&_code]:bg-hover [&_code]:px-1 [&_code]:text-[0.8125rem] [&_blockquote]:my-2 [&_blockquote]:border-l-2 [&_blockquote]:border-border [&_blockquote]:pl-3 [&_blockquote]:text-secondary [&_a]:text-accent [&_a]:underline">
      <Markdown
        skipHtml
        components={{
          // Never load remote images from a private note. Links are explicit
          // user actions through the same native handler as message links.
          img: ({ alt }) => <span>{alt}</span>,
          a: ({ href, children }) => (
            <a
              href={href}
              onAuxClick={(event) => event.preventDefault()}
              onClick={(event) => {
                event.preventDefault()
                if (href && /^(https?:|mailto:)/i.test(href)) openExternal(href)
              }}
            >
              {children}
            </a>
          ),
        }}
      >
        {text}
      </Markdown>
    </div>
  )
}
