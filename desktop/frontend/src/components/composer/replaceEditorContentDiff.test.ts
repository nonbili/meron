import { it, expect } from 'bun:test'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { replaceEditorContentDiff } from './composerHelpers'
it('keeps typing undoable across a signature switch, and the switch out of the history', () => {
  const editor = new Editor({ extensions: [StarterKit], content: '<p>Hi</p><p></p><p>Me</p><p>Phone</p>' })
  editor.commands.insertContentAt(3, ' there')
  replaceEditorContentDiff(editor, '<p>Hi there</p>')
  expect(editor.getHTML()).toBe('<p>Hi there</p>')
  editor.commands.undo()
  expect(editor.getHTML()).toBe('<p>Hi</p>')
  editor.commands.redo()
  replaceEditorContentDiff(editor, '<p>Hi there</p><p></p><p>Me</p><p>Phone</p>')
  expect(editor.getHTML()).toBe('<p>Hi there</p><p></p><p>Me</p><p>Phone</p>')
  editor.commands.undo()
  expect(editor.getHTML()).toBe('<p>Hi</p><p></p><p>Me</p><p>Phone</p>')
  // signature above a forwarded block
  const e2 = new Editor({ extensions: [StarterKit], content: '<p>FYI</p><p></p><p>Me</p><p>Fwd</p><p>Me</p>' })
  replaceEditorContentDiff(e2, '<p>FYI</p><p>Fwd</p><p>Me</p>')
  expect(e2.getHTML()).toBe('<p>FYI</p><p>Fwd</p><p>Me</p>')
  replaceEditorContentDiff(e2, '<p>FYI</p><p></p><p>Me</p><p>Fwd</p><p>Me</p>')
  expect(e2.getHTML()).toBe('<p>FYI</p><p></p><p>Me</p><p>Fwd</p><p>Me</p>')
})
