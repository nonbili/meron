import { useEffect, useState, type FormEvent } from 'react'
import { invoke } from '../../lib/bridge'
import { useTranslation } from '../../lib/i18n'

type Card = { id: string; version: string; name: string; emails: string[] }
type Page = { contacts: Card[]; more: boolean }
type Draft = { card?: Card; creationId?: string; name: string; emails: string }
const button = 'rounded border border-border px-2 py-1 disabled:opacity-50'
const field = 'w-full rounded border border-border bg-transparent px-2 py-1'

export function BookContacts({
  bookId,
  onChange,
  disabled,
  onBusy,
}: {
  bookId: string
  onChange: () => void
  disabled: boolean
  onBusy?: (busy: boolean) => void
}) {
  const { t } = useTranslation()
  const label = (key: string, defaultValue: string) => t(`contacts.book.${key}`, { defaultValue })
  const [page, setPage] = useState<Page>({ contacts: [], more: false })
  const [query, setQuery] = useState('')
  const [offset, setOffset] = useState(0)
  const [reload, setReload] = useState(0)
  const [draft, setDraft] = useState<Draft>()
  const [confirm, setConfirm] = useState(false)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')

  useEffect(() => {
    if (disabled) return
    let active = true
    void invoke<Page>('carddav.contacts', { book_id: bookId, query, offset })
      .then((result) => {
        if (active) setPage(result)
      })
      .catch((err) => {
        if (active) setMessage(String(err))
      })
    return () => {
      active = false
    }
  }, [bookId, query, offset, reload, disabled])

  function edit(card?: Card) {
    setConfirm(false)
    setMessage('')
    setDraft({
      card,
      creationId: card ? undefined : crypto.randomUUID(),
      name: card?.name ?? '',
      emails: card?.emails.join('\n') ?? '',
    })
  }

  async function mutate(remove: boolean) {
    if (!draft || busy || disabled) return
    const card = draft.card
    setBusy(true)
    onBusy?.(true)
    setMessage('')
    try {
      const result = await invoke<{ sync_error?: string }>(
        remove ? 'carddav.deleteContact' : card ? 'carddav.updateContact' : 'carddav.createContact',
        {
          book_id: bookId,
          ...(draft.creationId ? { creation_id: draft.creationId } : {}),
          ...(card ? { contact_id: card.id, version: card.version } : {}),
          ...(remove
            ? {}
            : {
                name: draft.name,
                emails: draft.emails
                  .split('\n')
                  .map((line) => line.trim())
                  .filter(Boolean),
              }),
        },
      )
      setDraft(undefined)
      setConfirm(false)
      setOffset(0)
      setReload((n) => n + 1)
      onChange()
      if (result.sync_error)
        setMessage(
          label('refreshWarning', 'The change was saved on the server, but the list could not refresh. Use Sync now.'),
        )
    } catch (err) {
      setMessage(String(err))
      setConfirm(false)
    } finally {
      setBusy(false)
      onBusy?.(false)
    }
  }

  function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!confirm) void mutate(false)
  }

  return (
    <div className="space-y-2 rounded border border-border p-2">
      {message && (
        <p role="alert" className="break-words">
          {message}
        </p>
      )}
      {draft ? (
        <form onSubmit={save} className="space-y-2">
          <fieldset disabled={busy || disabled} className="space-y-2">
            <label className="block">
              {label('name', 'Name')}
              <input
                className={field}
                required={!draft.card || !!draft.card.name}
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
              />
            </label>
            <label className="block">
              {label('emails', 'Email addresses (one per line)')}
              <textarea
                className={field}
                rows={3}
                value={draft.emails}
                onChange={(event) => setDraft({ ...draft, emails: event.target.value })}
              />
            </label>
            {confirm && <p>{label('confirm', 'Delete this contact from the server?')}</p>}
            <div className="flex flex-wrap gap-2">
              {!confirm && (
                <button type="submit" className={button}>
                  {label('save', 'Save contact')}
                </button>
              )}
              {draft.card && (
                <button
                  type="button"
                  className={button}
                  onClick={() => (confirm ? void mutate(true) : setConfirm(true))}
                >
                  {confirm ? label('confirmDelete', 'Confirm delete') : label('delete', 'Delete contact')}
                </button>
              )}
              <button
                type="button"
                className={button}
                onClick={() => {
                  if (confirm) setConfirm(false)
                  else {
                    setDraft(undefined)
                    setMessage('')
                  }
                }}
              >
                {label('cancel', 'Cancel')}
              </button>
            </div>
          </fieldset>
        </form>
      ) : (
        <>
          <button type="button" className={button} disabled={disabled} onClick={() => edit()}>
            {label('new', 'New contact')}
          </button>
          <input
            className={field}
            aria-label={label('search', 'Search address book')}
            placeholder={label('search', 'Search address book')}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value)
              setOffset(0)
            }}
          />
          {page.contacts.map((card) => (
            <button
              key={card.id}
              type="button"
              disabled={disabled}
              className="block w-full break-words rounded px-2 py-1 text-left hover:bg-hover"
              onClick={() => edit(card)}
            >
              {card.name || card.emails[0] || label('unnamed', 'Unnamed contact')}
              {card.name && card.emails.length > 0 && (
                <span className="block text-xs text-muted-foreground">{card.emails.join(', ')}</span>
              )}
            </button>
          ))}
          {(offset > 0 || page.more) && (
            <div className="flex gap-2">
              <button type="button" className={button} disabled={offset === 0} onClick={() => setOffset(offset - 50)}>
                {label('previous', 'Previous')}
              </button>
              <button type="button" className={button} disabled={!page.more} onClick={() => setOffset(offset + 50)}>
                {label('next', 'Next')}
              </button>
            </div>
          )}
        </>
      )}
    </div>
  )
}
