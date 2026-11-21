import { useState } from 'react'
import { Plus, Trash2, X } from 'lucide-react'
import { invoke } from '../../lib/bridge'
import { errorMessage } from '../../lib/errors'
import { useTranslation } from '../../lib/i18n'
import { confirmAction, showToast } from '../../states/ui'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { SelectInput, TextInput } from '../field/Field'

/** A contact saved in a synced address book (see `carddav.cardsFor`). */
export type BookCard = {
  book_id: string
  book_name: string
  id: string
  version: string
  name: string
  emails: string[]
  /** The address book account's login. */
  account: string
  google: boolean
  /** The vCard NOTE, shared with other clients. */
  note: string
}

/** An enabled address book, with the login of the account it is in. */
export type AddressBook = { id: string; name: string; account: string }

/** "Work · cora@example.test": which server account a book is on. */
export const bookLabel = (name: string, account: string) => (account ? `${name} · ${account}` : name)

export type EditorResult = { name: string; emails: string[]; note: string } | null

type Translate = ReturnType<typeof useTranslation>['t']

function confirmDeleteCard(card: BookCard, translate: Translate) {
  const label = translate('contacts.editorDelete', { defaultValue: labels.Delete })
  return confirmAction({
    title: label,
    message: translate('contacts.editorDeleteConfirm', {
      name: card.name || card.emails[0] || '',
      book: card.book_name,
      defaultValue: 'Delete {name} from {book}? This removes it from the server.',
    }),
    confirmLabel: label,
    tone: 'danger',
  })
}

/** Ask, then delete a saved contact from its address book's server.
 * Resolves to whether it was deleted. */
export async function deleteBookCard(card: BookCard, translate: Translate) {
  if (!(await confirmDeleteCard(card, translate))) return false
  try {
    const result = await invoke<{ sync_error?: string }>('carddav.deleteContact', {
      book_id: card.book_id,
      contact_id: card.id,
      version: card.version,
    })
    if (result?.sync_error)
      showToast(translate('contacts.editorRefreshWarning', { defaultValue: labels.RefreshWarning }), 'error')
    return true
  } catch (err) {
    showToast(errorMessage(err, translate('contacts.editorFailed', { defaultValue: labels.Failed })), 'error')
    return false
  }
}

/** Create a contact in an address book, or edit or delete one, on the server. */
export function ContactEditor({
  card,
  books,
  initialName = '',
  initialEmail = '',
  initialNote,
  initialBookId,
  onDone,
  onCancel,
}: {
  card?: BookCard
  /** Where a new contact can go; enabled address books only. */
  books: AddressBook[]
  initialName?: string
  initialEmail?: string
  /** Prefills the note, e.g. with one kept only on this device. */
  initialNote?: string
  /** The address book a new contact goes to unless another is picked. */
  initialBookId?: string
  /** After the server took the change: what was saved, or null when deleted. */
  onDone: (result: EditorResult) => void
  onCancel: () => void
}) {
  const { t: translate } = useTranslation()
  const t = (key: string) => translate(`contacts.editor${key}`, { defaultValue: labels[key] })
  const [name, setName] = useState(card?.name ?? initialName)
  const [emails, setEmails] = useState(card?.emails.length ? card.emails : [initialEmail])
  const [note, setNote] = useState(initialNote ?? card?.note ?? '')
  const [bookId, setBookId] = useState(
    card?.book_id ?? books.find((book) => book.id === initialBookId)?.id ?? books[0]?.id ?? '',
  )
  // One per new contact, so a retried create cannot make a second card.
  const [creationId] = useState(() => (card ? undefined : crypto.randomUUID()))
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const cleanEmails = emails.map((email) => email.trim()).filter(Boolean)
  // Contacts are reached by address here, so one without any would vanish.
  const canSave = !busy && !!bookId && cleanEmails.length > 0 && (!!name.trim() || (!!card && !card.name))

  async function send(method: string, params: Record<string, unknown>) {
    setBusy(true)
    setError('')
    try {
      const result = (await invoke<{ sync_error?: string }>(method, { book_id: bookId, ...params })) ?? {}
      if (result.sync_error) showToast(t('RefreshWarning'), 'error')
      return result
    } catch (err) {
      setError(errorMessage(err, t('Failed')))
      return null
    } finally {
      setBusy(false)
    }
  }

  async function save() {
    if (!canSave) return
    const result = await send(
      card ? 'carddav.updateContact' : 'carddav.createContact',
      card
        ? {
            contact_id: card.id,
            version: card.version,
            name: name.trim(),
            emails: cleanEmails,
            // Only a changed note is sent: the server's NOTE lines (several,
            // or with parameters) stay exactly as they are otherwise.
            ...(note.trim() !== card.note.trim() ? { note: note.trim() } : {}),
          }
        : { creation_id: creationId, name: name.trim(), emails: cleanEmails, note: note.trim() },
    )
    if (result) onDone({ name: name.trim(), emails: cleanEmails, note: note.trim() })
  }

  async function remove() {
    if (!card || busy) return
    if (!(await confirmDeleteCard(card, translate))) return
    const result = await send('carddav.deleteContact', { contact_id: card.id, version: card.version })
    if (result) onDone(null)
  }

  return (
    <form
      className="flex min-h-0 flex-1 flex-col"
      onSubmit={(event) => {
        event.preventDefault()
        void save()
      }}
    >
      <fieldset disabled={busy} className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4">
        <label className="flex flex-col gap-1.5">
          <span className="px-0.5 text-[0.6875rem] font-semibold text-secondary">{t('Name')}</span>
          <TextInput fieldSize="md" autoFocus value={name} onChange={(event) => setName(event.target.value)} />
        </label>
        <div className="flex flex-col gap-1.5">
          <span id="contact-editor-emails" className="px-0.5 text-[0.6875rem] font-semibold text-secondary">
            {t('Emails')}
          </span>
          {emails.map((email, index) => (
            <div key={index} className="flex items-center gap-1.5">
              <TextInput
                fieldSize="md"
                type="email"
                className="min-w-0 flex-1"
                aria-labelledby="contact-editor-emails"
                placeholder="name@example.com"
                value={email}
                onChange={(event) => setEmails(emails.map((old, i) => (i === index ? event.target.value : old)))}
                autoCapitalize="off"
                spellCheck={false}
              />
              {emails.length > 1 && (
                <IconButton
                  icon={X}
                  size="sm"
                  label={t('RemoveEmail')}
                  onClick={() => setEmails(emails.filter((_, i) => i !== index))}
                />
              )}
            </div>
          ))}
          <div>
            <Button variant="ghost" size="sm" leftIcon={Plus} onClick={() => setEmails([...emails, ''])}>
              {t('AddEmail')}
            </Button>
          </div>
        </div>
        <div className="flex flex-col gap-1.5">
          <label className="flex flex-col gap-1.5">
            <span className="px-0.5 text-[0.6875rem] font-semibold text-secondary">{t('Note')}</span>
            <textarea
              className="min-h-28 w-full resize-y rounded-lg border border-border bg-chats px-3 py-2 text-sm leading-relaxed outline-none placeholder:text-secondary/70 focus:border-accent/50 focus:ring-2 focus:ring-accent/10"
              placeholder={translate('contacts.notesPlaceholder')}
              value={note}
              onChange={(event) => setNote(event.target.value)}
            />
          </label>
          <span className="px-0.5 text-[0.6875rem] text-secondary">{t('NoteHint')}</span>
        </div>
        {card ? (
          <p className="px-0.5 text-[0.6875rem] text-secondary">
            {translate('contacts.editorSavedIn', {
              book: bookLabel(card.book_name, card.account),
              defaultValue: 'Saved in {book}',
            })}
          </p>
        ) : (
          books.length > 1 && (
            <label className="flex flex-col gap-1.5">
              <span className="px-0.5 text-[0.6875rem] font-semibold text-secondary">{t('Book')}</span>
              <SelectInput fieldSize="md" value={bookId} onChange={(event) => setBookId(event.target.value)}>
                {books.map((book) => (
                  <option key={book.id} value={book.id}>
                    {bookLabel(book.name, book.account)}
                  </option>
                ))}
              </SelectInput>
            </label>
          )
        )}
        {error && (
          <p role="alert" className="break-words px-0.5 text-xs text-rose-500 select-text">
            {error}
          </p>
        )}
      </fieldset>
      <div className="flex shrink-0 items-center gap-2 border-t border-border/50 px-3 py-2.5">
        {card && (
          <IconButton
            icon={Trash2}
            size="sm"
            label={t('Delete')}
            className="text-rose-600 hover:text-rose-600 dark:text-rose-400"
            disabled={busy}
            onClick={() => void remove()}
          />
        )}
        <Button variant="secondary" size="sm" className="ms-auto" onClick={onCancel} disabled={busy}>
          {translate('buttons.cancel')}
        </Button>
        <Button type="submit" size="sm" disabled={!canSave}>
          {busy ? t('Saving') : translate('buttons.save')}
        </Button>
      </div>
    </form>
  )
}

const labels: Record<string, string> = {
  Name: 'Name',
  Emails: 'Email addresses',
  AddEmail: 'Add email',
  RemoveEmail: 'Remove email',
  Book: 'Save to',
  Note: 'Note',
  NoteHint: 'Markdown supported. Saved in the address book with the contact.',
  Delete: 'Delete contact',
  Saving: 'Saving…',
  Failed: 'Could not save the contact.',
  RefreshWarning: 'The change was saved on the server, but contacts could not refresh. Use Sync now in Address books.',
}
