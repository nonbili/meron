import { observable } from '@legendapp/state'
import type { Contact } from '../types'
import { persistedField } from '../lib/sessionPref'
import { ui$ } from './ui'
import { invoke } from '../lib/bridge'
import { WRITE_SESSION } from './settings'
import { isNarrowWindow } from '../lib/useNarrowWindow'

export const contactKey = (addr: string) => addr.trim().toLowerCase()
/** The order of the Contacts panel's list; the core's `sort` parameter. */
export type ContactSort = 'frequent' | 'name' | 'recent'
export const CONTACT_SORTS: ContactSort[] = ['frequent', 'recent', 'name']
export const contacts$ = observable({
  selected: null as Contact | null,
  pinned: [] as Contact[],
  sort: 'frequent' as ContactSort,
})

export type ContactNote = {
  saved: string
  draft: string
  loaded: boolean
  loading: boolean
  saving: boolean
  error: boolean
}
export const contactNotes$ = observable({} as Record<string, ContactNote>)
const notePrefKey = (addr: string) => `contact_note:${contactKey(addr)}`

export async function loadContactNote(addr: string) {
  const key = contactKey(addr)
  if (contactNotes$[key].peek()?.loaded || contactNotes$[key].peek()?.loading) return
  contactNotes$[key].set({ saved: '', draft: '', loaded: false, loading: true, saving: false, error: false })
  try {
    const pref = notePrefKey(key)
    const result = await invoke<{ prefs: Record<string, unknown> }>('contacts.noteGet', { keys: [pref] })
    const text = typeof result.prefs[pref] === 'string' ? (result.prefs[pref] as string) : ''
    contactNotes$[key].set({ saved: text, draft: text, loaded: true, loading: false, saving: false, error: false })
  } catch {
    contactNotes$[key].loading.set(false)
    contactNotes$[key].error.set(true)
  }
}

/** Keep the draft while switching contacts or closing the panel. Only a
 * confirmed write becomes the saved version; failures leave the draft intact. */
export async function saveContactNote(addr: string) {
  const key = contactKey(addr)
  const note = contactNotes$[key].peek()
  if (!note?.loaded || note.saving) return
  const text = note.draft
  contactNotes$[key].saving.set(true)
  contactNotes$[key].error.set(false)
  try {
    await invoke('contacts.noteSet', { key: notePrefKey(key), value: text })
    contactNotes$[key].saved.set(text)
  } catch {
    contactNotes$[key].error.set(true)
  } finally {
    contactNotes$[key].saving.set(false)
  }
}

const pins = persistedField(
  contacts$.pinned,
  'pinned_contacts',
  (raw) => {
    if (!Array.isArray(raw)) return undefined
    const unique = new Map<string, Contact>()
    for (const value of raw) {
      if (typeof value?.addr !== 'string' || !value.addr.trim()) continue
      const addr = contactKey(value.addr)
      unique.set(addr, { addr, name: typeof value.name === 'string' ? value.name : '' })
    }
    return [...unique.values()]
  },
  { session: WRITE_SESSION },
)
const sort = persistedField(
  contacts$.sort,
  'contacts_sort',
  (raw) => (CONTACT_SORTS.includes(raw as ContactSort) ? (raw as ContactSort) : undefined),
  { session: WRITE_SESSION },
)
export const CONTACTS_PREF_KEYS = [pins.key, sort.key]
export function restoreContacts(prefs: Record<string, unknown>) {
  pins.restore(prefs)
  sort.restore(prefs)
}

export function toggleContactPin(contact: Contact) {
  const key = contactKey(contact.addr)
  const current = contacts$.pinned.peek()
  contacts$.pinned.set(
    current.some((c) => contactKey(c.addr) === key)
      ? current.filter((c) => contactKey(c.addr) !== key)
      : [...current, { ...contact, addr: key }],
  )
}

export function openContact(contact?: Contact) {
  contacts$.selected.set(contact ? { ...contact, addr: contactKey(contact.addr) } : null)
  ui$.tasksPanelOpen.set(false)
  ui$.calendarPanelOpen.set(false)
  ui$.contactsOpen.set(true)
}
export function toggleContactsPanel() {
  if (ui$.contactsOpen.peek()) ui$.contactsOpen.set(false)
  else openContact()
}

ui$.tasksPanelOpen.onChange(({ value }) => {
  if (value) ui$.contactsOpen.set(false)
})
ui$.calendarPanelOpen.onChange(({ value }) => {
  if (value) ui$.contactsOpen.set(false)
})

// On a narrow window Contacts owns the main surface, not a side slot. Make
// way for explicit navigation without disturbing the panel on wider windows.
export function leaveNarrowContacts() {
  if (isNarrowWindow()) ui$.contactsOpen.set(false)
}
