import { useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import {
  ArrowUpDown,
  BookUser,
  Check,
  ChevronLeft,
  Cloud,
  CloudUpload,
  Copy,
  Lock,
  Mail,
  Pencil,
  Plus,
  Pin,
  PinOff,
  SquarePen,
  Trash2,
  UserPlus,
  Users,
  X,
} from 'lucide-react'
import { invoke } from '../../lib/bridge'
import { errorMessage } from '../../lib/errors'
import { useTranslation } from '../../lib/i18n'
import { copyText } from '../../lib/native'
import { formatContact } from '../../lib/contacts'
import { folderLabel } from '../../lib/kanbanData'
import { accountOwning, accounts$, isSendableAccount } from '../../states/accounts'
import { openComposeTab, openThreadTab } from '../../states/compose'
import type { Contact, Message } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { ThreadListItem } from '../threads/ThreadListItem'
import {
  CONTACT_SORTS,
  clearContactNote,
  contactKey,
  contactNotes$,
  contacts$,
  loadContactNote,
  toggleContactPin,
} from '../../states/contacts'
import type { ContactSort } from '../../states/contacts'
import { MenuItem } from '../menu/MenuItem'
import { MenuSurface } from '../menu/MenuSurface'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'
import { confirmAction, showToast, ui$ } from '../../states/ui'
import { mail$ } from '../../states/mail'
import { IconButton } from '../button/IconButton'
import { useNarrowWindow } from '../../lib/useNarrowWindow'
import { ContactNotes, NoteMarkdown } from './ContactNotes'
import { AddressBooks } from './AddressBooks'
import {
  ContactEditor,
  bookLabel,
  deleteBookCard,
  type AddressBook,
  type BookCard,
  type EditorResult,
} from './ContactEditor'
import { Button } from '../button/Button'

const SORT_LABELS = {
  frequent: 'contacts.sortFrequent',
  recent: 'contacts.sortRecent',
  name: 'contacts.sortName',
} as const satisfies Record<ContactSort, string>

// This directory and its history are local: opening Contacts never changes the
// mailbox query, filters or folder, and never merges unrelated email threads.
export function ContactsView() {
  const { t } = useTranslation()
  const narrow = useNarrowWindow()
  const accounts = useValue(accounts$)
  const canSend = accounts.some(isSendableAccount)
  const foldersByAccount = useValue(mail$.foldersByAccount)
  const currentFolders = useValue(mail$.folders)
  const [copied, setCopied] = useState(false)
  const sort = useValue(contacts$.sort)
  const [sortOpen, setSortOpen] = useState(false)
  const [addressBooks, setAddressBooks] = useState(false)
  const sortRef = useRef<HTMLDivElement>(null)
  useDismissOnOutside(
    sortOpen,
    (target) => Boolean(sortRef.current?.contains(target as Node)),
    () => setSortOpen(false),
  )
  const [query, setQuery] = useState('')
  const [limit, setLimit] = useState(100)
  const [contacts, setContacts] = useState<Contact[]>([])
  const selected = useValue(contacts$.selected)
  // Our own address is drawn as the account, as the reader draws our messages.
  const ownAccount = accountOwning(accounts, selected?.addr)
  const selectedAddr = selected?.addr ?? ''
  const pinned = useValue(contacts$.pinned)
  const [threads, setThreads] = useState<Message[]>([])
  const [cursor, setCursor] = useState('')
  const [loading, setLoading] = useState(false)
  const [contactsLoading, setContactsLoading] = useState(true)
  const [error, setError] = useState(false)
  const [contactsError, setContactsError] = useState(false)
  const [retry, setRetry] = useState(0)
  const [page, setPage] = useState('')
  const [loadedAddr, setLoadedAddr] = useState('')
  const historyAddr = useRef('')
  // Enabled address books (where a new contact can go), and the cards saved
  // in them for the open contact's address.
  const [books, setBooks] = useState<AddressBook[]>([])
  const [cards, setCards] = useState<BookCard[]>([])
  // The address whose cards `cards` holds, so "only in your mail" is not
  // shown before they have loaded.
  const [cardsAddr, setCardsAddr] = useState('')
  // The address whose card lookup failed: neither "only in your mail" nor
  // its cards can be claimed then.
  const [cardsFailed, setCardsFailed] = useState('')
  const [editor, setEditor] = useState<null | {
    card?: BookCard
    name?: string
    email?: string
    note?: string
    /** The note is the one kept on this device: drop that copy once saved. */
    movesLocalNote?: boolean
  }>(null)
  // The note kept only on this device for the open contact. Once the contact
  // is in an address book, its note lives on the card instead.
  const localNoteState = useValue(contactNotes$[contactKey(selectedAddr)])
  const localNote = localNoteState?.loaded ? localNoteState.saved.trim() : ''
  useEffect(() => {
    if (selectedAddr) void loadContactNote(selectedAddr)
  }, [selectedAddr])

  const localNoteEditing = !!localNoteState && localNoteState.draft !== localNoteState.saved
  /** Once confirmed, add the device note to a card's note on the server,
   * then drop it here. Whoever can read that address book will see it. */
  async function moveLocalNote(addr: string, card: BookCard, text: string) {
    const confirmed = await confirmAction({
      title: t('contacts.moveNoteTitle', { defaultValue: 'Move note' }),
      message: t('contacts.moveNoteConfirm', {
        book: bookLabel(card.book_name, card.account),
        defaultValue:
          'Move this note to {book}? It is added to the contact there, and anyone with access to that address book will see it.',
      }),
      confirmLabel: t('contacts.moveNoteConfirmLabel', { defaultValue: 'Move' }),
    })
    if (!confirmed) return
    try {
      // Always written, version-checked, even if the cached note seems to
      // hold it already: the cache may be behind the server, and a repeated
      // note is better than a lost one.
      const result = await invoke<{ sync_error?: string }>('carddav.updateContact', {
        book_id: card.book_id,
        contact_id: card.id,
        version: card.version,
        name: card.name,
        emails: card.emails,
        note: card.note.trim() ? `${card.note.trim()}\n\n${text}` : text,
      })
      // Saved on the server even when the refresh after it failed; only the
      // copy here is behind until the next sync.
      if (result?.sync_error)
        showToast(
          t('contacts.editorRefreshWarning', {
            defaultValue:
              'The change was saved on the server, but contacts could not refresh. Use Sync now in Address books.',
          }),
          'error',
        )
      await clearContactNote(addr, text)
    } catch (err) {
      showToast(
        errorMessage(
          err,
          t('contacts.moveNoteFailed', { defaultValue: 'Could not save the note to the address book.' }),
        ),
        'error',
      )
    } finally {
      setRetry((n) => n + 1)
    }
  }

  useEffect(() => {
    let cancelled = false
    void invoke<{
      accounts: { username: string; books: { id: string; name: string; enabled: boolean }[] }[]
    }>('carddav.accounts')
      .then((res) => {
        if (!cancelled)
          setBooks(
            (res?.accounts ?? []).flatMap((account) =>
              account.books
                .filter((book) => book.enabled)
                .map(({ id, name }) => ({
                  id,
                  name,
                  account: account.username,
                })),
            ),
          )
      })
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [retry])

  // Cards show only once `cardsAddr` names the open address, so another
  // contact's never do; a refresh keeps the current ones until it answers.
  useEffect(() => {
    setCardsFailed('')
    if (!selectedAddr) return
    let cancelled = false
    void invoke<{ cards: BookCard[] }>('carddav.cardsFor', { email: selectedAddr })
      .then((res) => {
        if (cancelled) return
        setCards(res?.cards ?? [])
        setCardsAddr(selectedAddr)
      })
      .catch(() => {
        if (!cancelled) setCardsFailed(selectedAddr)
      })
    return () => {
      cancelled = true
    }
  }, [selectedAddr, retry])

  // The open editor, for a save that finishes after it was left with Back.
  const openEditor = useRef(editor)
  openEditor.current = editor

  function editorDone(result: EditorResult) {
    const editing = editor?.card
    if (result && editor?.movesLocalNote && editor.note) void clearContactNote(selectedAddr, editor.note)
    setRetry((n) => n + 1)
    // Left before the server answered: the change is saved, but whatever is
    // open now (another editor, another contact) stays as it is.
    if (openEditor.current !== editor) return
    setEditor(null)
    // Deleted: stay, as the address may still be in mail.
    if (!result) return
    const addr = editing || selectedAddr ? selectedAddr : result.emails[0]
    if (addr && result.emails.some((email) => contactKey(email) === contactKey(addr))) {
      contacts$.selected.set({ name: result.name, addr })
    } else if (editing) {
      contacts$.selected.set(null)
    }
  }

  useEffect(() => {
    let cancelled = false
    setContactsLoading(true)
    setContactsError(false)
    const timer = setTimeout(() => {
      void invoke<{ contacts: Contact[] }>('mail.suggestContacts', { account: '', query, limit, sort, directory: true })
        .then((res) => {
          if (!cancelled) setContacts(res.contacts)
        })
        .catch(() => {
          if (!cancelled) setContactsError(true)
        })
        .finally(() => {
          if (!cancelled) setContactsLoading(false)
        })
    }, 150)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [query, limit, sort, retry, accounts])

  useEffect(() => {
    if (!selected) return
    if (historyAddr.current !== selected.addr) {
      historyAddr.current = selected.addr
      setThreads([])
      setCursor('')
      setLoadedAddr('')
      if (page) {
        setPage('')
        return
      }
    }
    let cancelled = false
    setLoading(true)
    setError(false)
    void invoke<{ threads: Message[]; next_cursor?: string }>('mail.contactThreads', {
      addr: selected.addr,
      before_cursor: page.startsWith(`${selected.addr}\n`) ? page.slice(selected.addr.length + 1) : '',
      limit: 50,
    })
      .then((res) => {
        if (cancelled) return
        setThreads((old) => [
          ...new Map(
            (page.startsWith(`${selected.addr}\n`) ? [...old, ...res.threads] : res.threads).map((thread) => [
              thread.thread_id,
              thread,
            ]),
          ).values(),
        ])
        setCursor(res.next_cursor ?? '')
        setLoadedAddr(selected.addr)
      })
      .catch(() => {
        if (!cancelled) setError(true)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [selectedAddr, page, retry])

  const retryButton = (
    <button className="p-4 text-sm text-accent" onClick={() => setRetry(retry + 1)}>
      {t('contacts.retry')}
    </button>
  )
  const isPinned = (contact: Contact) => pinned.some((c) => contactKey(c.addr) === contactKey(contact.addr))
  const matchingPins = pinned
    .filter((c) => `${c.name} ${c.addr}`.toLowerCase().includes(query.trim().toLowerCase()))
    .sort((a, b) => (a.name || a.addr).localeCompare(b.name || b.addr))
  const others = contacts.filter((c) => !isPinned(c))
  const contactRow = (contact: Contact) => (
    <div key={contactKey(contact.addr)} className="group mb-0.5 flex items-center rounded-lg pr-1.5 hover:bg-hover">
      <button
        className="flex min-w-0 flex-1 items-center gap-2.5 p-2 text-left cursor-pointer"
        onClick={() => {
          contacts$.selected.set(contact)
          setThreads([])
          setCursor('')
          setPage('')
        }}
      >
        <Avatar name={contact.name || contact.addr} email={contact.addr} size={32} />
        <span className="min-w-0">
          <span className="block truncate text-sm font-medium">{contact.name || contact.addr}</span>
          {contact.name && contact.name !== contact.addr && (
            <span className="block truncate text-xs text-secondary">{contact.addr}</span>
          )}
        </span>
      </button>
      {/* Not an everyday action, so it stays out of sight until the row is hovered. */}
      <IconButton
        icon={isPinned(contact) ? PinOff : Pin}
        size="sm"
        className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
        label={t(isPinned(contact) ? 'contacts.unpin' : 'contacts.pin')}
        onClick={() => toggleContactPin(contact)}
      />
    </div>
  )
  const copyButton = selected && (
    <button
      className="shrink-0 rounded p-0.5 text-secondary opacity-0 transition-opacity group-hover:opacity-100 hover:text-primary focus-visible:opacity-100 cursor-pointer"
      aria-label={t(copied ? 'common.copied' : 'chat.copyEmailAddress')}
      title={t(copied ? 'common.copied' : 'chat.copyEmailAddress')}
      onClick={() => {
        copyText(selected.addr)
          .then(() => {
            setCopied(true)
            setTimeout(() => setCopied(false), 1500)
          })
          .catch(() => undefined)
      }}
    >
      {copied ? <Check size={12} className="text-accent" /> : <Copy size={12} />}
    </button>
  )
  return (
    <section className="flex min-h-0 min-w-0 w-full shrink-0 flex-col border-l border-border/60 bg-chats min-[901px]:w-[340px]">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b border-border/50 pl-3 pr-1.5 text-sm font-semibold">
        {editor ? (
          <IconButton icon={ChevronLeft} size="sm" label={t('buttons.back')} onClick={() => setEditor(null)} />
        ) : selected ? (
          <IconButton
            icon={ChevronLeft}
            size="sm"
            label={t('buttons.back')}
            onClick={() => {
              contacts$.selected.set(null)
              setPage('')
              setThreads([])
            }}
          />
        ) : (
          <Users size={17} className="text-accent" />
        )}
        <h1 className="min-w-0 flex-1 truncate">
          {editor
            ? editor.card
              ? t('contacts.editContact', { defaultValue: 'Edit contact' })
              : t('contacts.newContact', { defaultValue: 'New contact' })
            : t('contacts.title')}
        </h1>
        {!editor && !selected && !addressBooks && books.length > 0 && (
          <IconButton
            icon={UserPlus}
            size="sm"
            label={t('contacts.newContact', { defaultValue: 'New contact' })}
            onClick={() => setEditor({})}
          />
        )}
        {!editor && !selected && (
          <IconButton
            icon={BookUser}
            size="sm"
            label={t('contacts.addressBooks', { defaultValue: 'Address books' })}
            active={addressBooks}
            onClick={() => setAddressBooks(!addressBooks)}
          />
        )}
        {!editor && selected && (
          <IconButton
            icon={isPinned(selected) ? PinOff : Pin}
            size="sm"
            label={t(isPinned(selected) ? 'contacts.unpin' : 'contacts.pin')}
            onClick={() => toggleContactPin(selected)}
          />
        )}
        <IconButton icon={X} size="sm" label={t('buttons.close')} onClick={() => ui$.contactsOpen.set(false)} />
      </header>
      {editor ? (
        <ContactEditor
          key={editor.card?.id ?? 'new'}
          card={editor.card}
          initialName={editor.name}
          initialEmail={editor.email}
          initialNote={editor.note}
          books={books}
          onCancel={() => setEditor(null)}
          onDone={editorDone}
        />
      ) : !selected && addressBooks ? (
        <AddressBooks onChange={() => setRetry((n) => n + 1)} />
      ) : !selected ? (
        <>
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <div className="m-3 flex items-center gap-1.5">
              <input
                className="min-w-0 flex-1 rounded-lg border border-border bg-transparent px-3 py-2 text-sm"
                value={query}
                aria-label={t('contacts.search')}
                placeholder={t('contacts.search')}
                onChange={(event) => {
                  setQuery(event.target.value)
                  setLimit(100)
                }}
              />
              <div ref={sortRef} className="relative">
                <IconButton
                  icon={ArrowUpDown}
                  size="md"
                  radius="lg"
                  active={sortOpen}
                  label={`${t('contacts.sort')}: ${t(SORT_LABELS[sort])}`}
                  aria-haspopup="menu"
                  aria-expanded={sortOpen}
                  onClick={() => setSortOpen(!sortOpen)}
                />
                {sortOpen && (
                  <MenuSurface role="menu" className="absolute right-0 top-full z-50 mt-1 min-w-[10rem] p-1">
                    {CONTACT_SORTS.map((option) => (
                      <MenuItem
                        key={option}
                        label={t(SORT_LABELS[option])}
                        pressed={option === sort}
                        trailing={
                          option === sort ? <Check size={13} className="text-accent" /> : <span className="w-[13px]" />
                        }
                        onClick={() => {
                          contacts$.sort.set(option)
                          setLimit(100)
                          setSortOpen(false)
                        }}
                      />
                    ))}
                  </MenuSurface>
                )}
              </div>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto px-2">
              {matchingPins.length > 0 && (
                <>
                  <h2 className="px-2 py-2 text-xs font-semibold text-secondary">{t('contacts.pinned')}</h2>
                  {matchingPins.map(contactRow)}
                </>
              )}
              {contactsLoading && !contacts.length ? (
                <p className="p-3 text-sm text-secondary">{t('common.loading')}</p>
              ) : contactsError ? (
                retryButton
              ) : (
                <>
                  {matchingPins.length > 0 && others.length > 0 && (
                    <h2 className="px-2 py-2 text-xs font-semibold text-secondary">{t('contacts.other')}</h2>
                  )}
                  {others.map(contactRow)}
                  {!contacts.length && !matchingPins.length && (
                    <p className="p-3 text-sm text-secondary">{t('contacts.empty')}</p>
                  )}
                  {contacts.length >= limit && (
                    <button
                      disabled={contactsLoading}
                      className="p-3 text-sm text-accent"
                      onClick={() => setLimit(limit + 100)}
                    >
                      {t('threads.actions.loadMore')}
                    </button>
                  )}
                </>
              )}
            </div>
          </div>
        </>
      ) : (
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <div className="flex items-center gap-3 px-4 pt-4 pb-2">
            <Avatar
              name={ownAccount?.display_name || selected.name || selected.addr}
              email={ownAccount ? undefined : selected.addr}
              src={ownAccount?.avatar_url}
              size={48}
            />
            <div className="group min-w-0 flex-1">
              <div className="flex min-w-0 items-center gap-1">
                <h2 className="truncate text-base font-semibold">{selected.name || selected.addr}</h2>
                {!selected.name && copyButton}
              </div>
              {selected.name && (
                <div className="flex min-w-0 items-center gap-1">
                  <p className="truncate text-xs text-secondary select-text">{selected.addr}</p>
                  {copyButton}
                </div>
              )}
            </div>
            {canSend && (
              <IconButton
                icon={SquarePen}
                variant="accentSoft"
                size="md"
                radius="full"
                label={t('composer.actions.newMessage')}
                onClick={() => openComposeTab({ to: formatContact(selected) })}
              />
            )}
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-2">
            {cardsAddr === selected.addr && (
              <section className="pb-2">
                <h2 className="px-2 pt-1 pb-1.5 text-xs font-semibold text-secondary">
                  {t('contacts.addressBook', { defaultValue: 'Address book' })}
                </h2>
                {cards.map((card) => (
                  <div key={`${card.book_id}\n${card.id}`}>
                    <div className="group flex items-center gap-2.5 rounded-lg px-2 py-1.5 hover:bg-hover">
                      <BookUser size={16} aria-hidden className="shrink-0 text-accent" />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm">{card.book_name}</span>
                        <span className="block truncate text-xs text-secondary">
                          {card.google ? `Google Contacts · ${card.account}` : card.account}
                        </span>
                      </span>
                      <IconButton
                        icon={Pencil}
                        size="sm"
                        label={t('contacts.editContact', { defaultValue: 'Edit contact' })}
                        onClick={() => setEditor({ card })}
                      />
                      <IconButton
                        icon={Trash2}
                        size="sm"
                        label={t('contacts.editorDelete', { defaultValue: 'Delete contact' })}
                        className="text-rose-600 hover:text-rose-600 dark:text-rose-400"
                        onClick={() =>
                          void deleteBookCard(card, t).then((deleted) => {
                            if (deleted) setRetry((n) => n + 1)
                          })
                        }
                      />
                    </div>
                  </div>
                ))}
                {!cards.length && (
                  <div className="flex items-center gap-2.5 px-2 py-1.5">
                    <Mail size={16} aria-hidden className="shrink-0 text-secondary" />
                    <span className="min-w-0 flex-1 text-sm text-secondary">
                      {t('contacts.onlyInMail', { defaultValue: 'Only in your mail' })}
                    </span>
                    {books.length > 0 && (
                      <Button
                        variant="secondary"
                        size="sm"
                        leftIcon={Plus}
                        onClick={() =>
                          setEditor({
                            name: selected.name,
                            email: selected.addr,
                            note: localNote || undefined,
                            movesLocalNote: !!localNote,
                          })
                        }
                      >
                        {t('contacts.saveToAddressBook', { defaultValue: 'Save to address book' })}
                      </Button>
                    )}
                  </div>
                )}
              </section>
            )}
            {cardsFailed === selected.addr && (
              <button className="px-2 pb-2 text-xs text-accent" onClick={() => setRetry((n) => n + 1)}>
                {t('contacts.addressBookRetry', { defaultValue: 'Could not check the address book. Try again' })}
              </button>
            )}
            {(cardsAddr === selected.addr && !cards.length) || cardsFailed === selected.addr ? (
              <ContactNotes key={selected.addr} addr={selected.addr} />
            ) : (
              cardsAddr === selected.addr && (
                <>
                  {/* Saved in an address book: each place a note is kept says
                      so, and moving one up is the user's choice. */}
                  {(localNote || localNoteEditing) && (
                    <ContactNotes
                      key={selected.addr}
                      addr={selected.addr}
                      icon={<Lock size={12} aria-hidden className="shrink-0 text-secondary" />}
                      title={t('contacts.deviceNote', { defaultValue: 'Note only on this device' })}
                      footer={
                        <div className="mt-2 flex flex-wrap gap-1.5">
                          {cards.map((card) => (
                            <Button
                              key={`${card.book_id}\n${card.id}`}
                              variant="secondary"
                              size="sm"
                              leftIcon={CloudUpload}
                              title={bookLabel(card.book_name, card.account)}
                              onClick={() => void moveLocalNote(selected.addr, card, localNote)}
                            >
                              {t('contacts.moveNote', { book: card.book_name, defaultValue: 'Move to {book}' })}
                            </Button>
                          ))}
                        </div>
                      }
                    />
                  )}
                  {cards.map((card) => (
                    <BookNote key={`${card.book_id}\n${card.id}`} card={card} onEdit={() => setEditor({ card })} />
                  ))}
                </>
              )
            )}
            <h2 className="px-2 pt-4 pb-2 text-xs font-semibold text-secondary">{t('contacts.recentConversations')}</h2>
            {!selected ? (
              <p className="p-3 text-sm text-secondary">{t('contacts.select')}</p>
            ) : (
              <>
                {(loadedAddr === selected.addr ? threads : []).map((thread) => (
                  <ThreadListItem
                    key={thread.thread_id}
                    thread={thread}
                    badgeAccount={accounts.find((account) => account.id === thread.account_id)}
                    // History spans folders, so each row names where it lives.
                    folderLabel={folderLabel(
                      { accountId: thread.account_id, folderId: thread.folder_id },
                      [...(foldersByAccount[thread.account_id] ?? []), ...currentFolders],
                      accounts,
                      t,
                    )}
                    onSelect={() => {
                      openThreadTab(thread)
                      if (narrow) ui$.contactsOpen.set(false)
                    }}
                  />
                ))}
                {loading || (!error && loadedAddr !== selected.addr) ? (
                  <p className="p-3 text-sm text-secondary">{t('common.loading')}</p>
                ) : error ? (
                  retryButton
                ) : cursor ? (
                  <button className="p-3 text-sm text-accent" onClick={() => setPage(`${selected.addr}\n${cursor}`)}>
                    {t('threads.actions.loadMore')}
                  </button>
                ) : (
                  <>
                    {!threads.length && (
                      <p className="px-2 pt-1 text-sm text-secondary">{t('contacts.noConversations')}</p>
                    )}
                    <p className="px-2 pt-2 pb-1 text-[0.6875rem] leading-snug text-secondary/80">
                      {t('contacts.cachedHistory')}
                    </p>
                  </>
                )}
              </>
            )}
          </div>
        </div>
      )}
    </section>
  )
}

/** A contact's note as its address book keeps it, shared with other clients. */
function BookNote({ card, onEdit }: { card: BookCard; onEdit: () => void }) {
  const { t } = useTranslation()
  const label = bookLabel(card.book_name, card.account)
  return (
    <section className="border-b border-border/50 px-2 pt-1 pb-4" aria-label={label}>
      <div className="flex h-8 items-center gap-2">
        <Cloud size={12} aria-hidden className="shrink-0 text-secondary" />
        <h2 className="min-w-0 flex-1 truncate text-xs font-semibold text-secondary" title={label}>
          {label}
        </h2>
        {card.note && (
          <button
            className="-mr-1 inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-[0.6875rem] font-semibold text-secondary transition-colors hover:bg-hover hover:text-primary"
            aria-label={t('contacts.editNote', { defaultValue: 'Edit note' })}
            onClick={onEdit}
          >
            <Pencil size={12} className="stroke-[2.5]" />
            {t('contacts.notesEdit')}
          </button>
        )}
      </div>
      {card.note ? (
        <NoteMarkdown text={card.note} />
      ) : (
        <button
          className="flex w-full items-center gap-2 rounded-lg border border-dashed border-border px-3 py-2.5 text-left text-sm text-secondary transition-colors hover:border-accent/50 hover:text-accent"
          onClick={onEdit}
        >
          <Plus size={14} className="stroke-[2.5]" />
          {t('contacts.notesPlaceholder')}
        </button>
      )}
    </section>
  )
}
