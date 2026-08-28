import { describe, expect, it } from 'bun:test'
import { isPlainTextField, isRedoChord, isUndoChord, type EditChord } from './editUndo'
import { isMac } from './shortcuts'

const modKey = isMac ? { metaKey: true } : { ctrlKey: true }
const otherModKey = isMac ? { ctrlKey: true } : { metaKey: true }

const chord = (key: string, mods: Partial<EditChord> = {}): EditChord => ({
  key,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
})

const element = (tag: string, props: Record<string, unknown> = {}) =>
  ({ tagName: tag, isContentEditable: false, ...props }) as unknown as EventTarget

describe('isUndoChord', () => {
  it('matches the platform command key with Z', () => {
    expect(isUndoChord(chord('z', modKey))).toBe(true)
    expect(isUndoChord(chord('Z', modKey))).toBe(true)
  })

  it('ignores the non-native modifier, alone or held alongside', () => {
    expect(isUndoChord(chord('z', otherModKey))).toBe(false)
    expect(isUndoChord(chord('z', { ...modKey, ...otherModKey }))).toBe(false)
  })

  it('rejects the bare key, the redo chord and Alt variants', () => {
    expect(isUndoChord(chord('z'))).toBe(false)
    expect(isUndoChord(chord('z', { ...modKey, shiftKey: true }))).toBe(false)
    expect(isUndoChord(chord('z', { ...modKey, altKey: true }))).toBe(false)
  })
})

describe('isRedoChord', () => {
  it('matches mod+Shift+Z and mod+Y', () => {
    expect(isRedoChord(chord('z', { ...modKey, shiftKey: true }))).toBe(true)
    expect(isRedoChord(chord('Z', { ...modKey, shiftKey: true }))).toBe(true)
    expect(isRedoChord(chord('y', modKey))).toBe(true)
  })

  it('ignores the non-native modifier', () => {
    expect(isRedoChord(chord('z', { ...otherModKey, shiftKey: true }))).toBe(false)
    expect(isRedoChord(chord('y', { ...modKey, ...otherModKey }))).toBe(false)
  })

  it('does not match the plain undo chord', () => {
    expect(isRedoChord(chord('z', modKey))).toBe(false)
    expect(isRedoChord(chord('y'))).toBe(false)
  })
})

describe('isPlainTextField', () => {
  it('accepts textareas and text-like inputs', () => {
    expect(isPlainTextField(element('TEXTAREA'))).toBe(true)
    expect(isPlainTextField(element('INPUT', { type: 'text' }))).toBe(true)
    expect(isPlainTextField(element('INPUT', { type: 'search' }))).toBe(true)
    expect(isPlainTextField(element('INPUT', { type: 'email' }))).toBe(true)
  })

  it('leaves rich text, non-text inputs and other elements to their own handling', () => {
    // Tiptap keeps its own undo history.
    expect(isPlainTextField(element('DIV', { isContentEditable: true }))).toBe(false)
    expect(isPlainTextField(element('INPUT', { type: 'checkbox' }))).toBe(false)
    expect(isPlainTextField(element('DIV'))).toBe(false)
    expect(isPlainTextField(null)).toBe(false)
  })
})
