import { useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { ArrowUpDown, Check, ChevronLeft, Copy, Pin, PinOff, SquarePen, Users, X } from 'lucide-react'
import { invoke } from '../../lib/bridge'
import { useTranslation } from '../../lib/i18n'
import { copyText } from '../../lib/native'
import { formatContact } from '../../lib/contacts'
import { folderLabel } from '../../lib/kanbanData'
import { accounts$, isSendableAccount } from '../../states/accounts'
import { openComposeTab, openThreadTab } from '../../states/compose'
import type { Contact, Message } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { ThreadListItem } from '../threads/ThreadListItem'
import { CONTACT_SORTS, contactKey, contacts$, toggleContactPin } from '../../states/contacts'
import type { ContactSort } from '../../states/contacts'
import { MenuItem } from '../menu/MenuItem'
import { MenuSurface } from '../menu/MenuSurface'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'
import { ui$ } from '../../states/ui'
import { mail$ } from '../../states/mail'
import { IconButton } from '../button/IconButton'
import { useNarrowWindow } from '../../lib/useNarrowWindow'
import { ContactNotes } from './ContactNotes'

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
        {selected ? (
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
        <h1 className="min-w-0 flex-1 truncate">{t('contacts.title')}</h1>
        {selected && (
          <IconButton
            icon={isPinned(selected) ? PinOff : Pin}
            size="sm"
            label={t(isPinned(selected) ? 'contacts.unpin' : 'contacts.pin')}
            onClick={() => toggleContactPin(selected)}
          />
        )}
        <IconButton icon={X} size="sm" label={t('buttons.close')} onClick={() => ui$.contactsOpen.set(false)} />
      </header>
      {!selected ? (
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
            <Avatar name={selected.name || selected.addr} email={selected.addr} size={48} />
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
            <ContactNotes key={selected.addr} addr={selected.addr} />
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
