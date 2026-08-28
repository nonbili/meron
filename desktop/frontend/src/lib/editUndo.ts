// Undo/redo for plain text fields.
//
// WebKitGTK (the Linux webview) delivers Ctrl+Z to the page as an ordinary
// keydown and maps it to no editing command, so <input>/<textarea> fields have
// no undo there. WebKit does record the edits — queryCommandEnabled('undo')
// reports true and execCommand('undo') replays them — so running the command
// ourselves is enough. That keeps WebKit's own per-field history: keystroke
// coalescing, caret restoration, and a reset whenever the value is written
// programmatically (thread switch, draft hydration, clear-after-send), none of
// which a hand-rolled history would match.
//
// execCommand fires a real input event, so controlled React fields stay in sync
// through their normal onChange. Rich text keeps its own history (Tiptap) and is
// left alone.

import { isMac } from './shortcuts'

const TEXT_INPUT_TYPES = new Set(['text', 'search', 'url', 'tel', 'email', 'number', 'password'])

export type EditChord = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>

// The platform's command key, and the one that must not be held with it — same
// rule as chordFromEvent, so ⌃Z on macOS (where it is not undo, and ⌃Y is yank)
// and Super+Z elsewhere are left to their native meaning.
function hasCommandModifier(event: EditChord): boolean {
  if (isMac ? event.ctrlKey : event.metaKey) return false
  return isMac ? event.metaKey : event.ctrlKey
}

export function isUndoChord(event: EditChord): boolean {
  if (!hasCommandModifier(event) || event.altKey || event.shiftKey) return false
  return event.key === 'z' || event.key === 'Z'
}

export function isRedoChord(event: EditChord): boolean {
  if (!hasCommandModifier(event) || event.altKey) return false
  if (event.shiftKey) return event.key === 'z' || event.key === 'Z'
  return event.key === 'y' || event.key === 'Y'
}

export function isPlainTextField(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null
  if (!el || el.isContentEditable) return false
  if (el.tagName === 'TEXTAREA') return true
  if (el.tagName !== 'INPUT') return false
  return TEXT_INPUT_TYPES.has((el as HTMLInputElement).type)
}

// Returns whether the chord was handled, so the caller can stop dispatching.
export function handleEditUndoKeyDown(event: KeyboardEvent): boolean {
  if (!isPlainTextField(event.target)) return false
  const redo = isRedoChord(event)
  if (!redo && !isUndoChord(event)) return false
  event.preventDefault()
  document.execCommand(redo ? 'redo' : 'undo')
  return true
}
