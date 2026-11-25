import { expect, it } from 'bun:test'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { replaceEditorContentDiff } from './composerHelpers'
import { ComposerLink } from './composerLink'
import { linkAtCaret } from './LinkDialog'

const a = (text: string, href = 'https://x.example/a') =>
  `<a target="_blank" rel="noopener noreferrer nofollow" href="${href}">${text}</a>`

const editorWith = (html: string) =>
  new Editor({ extensions: [StarterKit.configure({ link: false }), ComposerLink], content: html })

// As typing does, one character at a time: each takes the stored marks, else
// those at the cursor.
const type = (editor: Editor, text: string) => {
  for (const character of text) editor.view.dispatch(editor.state.tr.insertText(character))
}
const caretAtEnd = (editor: Editor) => editor.commands.setTextSelection(editor.state.doc.content.size - 1)
const backspace = (editor: Editor) => {
  const at = editor.state.selection.from
  editor.view.dispatch(editor.state.tr.delete(at - 1, at))
}

it('keeps text typed before a link that opens a line out of the link', () => {
  const editor = editorWith(`<p>${a('https://x.example/a')}, hi</p>`)
  editor.commands.setTextSelection(1)
  type(editor, 'see ')
  expect(editor.getHTML()).toBe(`<p>see ${a('https://x.example/a')}, hi</p>`)
})

it('keeps text typed after a label out of it, as Gmail does', () => {
  const editor = editorWith(`<p>${a('Docs')}</p>`)
  caretAtEnd(editor)
  type(editor, ', see')
  expect(editor.getHTML()).toBe(`<p>${a('Docs')}, see</p>`)
})

it('keeps a URL link pointing where its text says as it is edited', () => {
  const editor = editorWith(`<p>${a('https://x.example/ab', 'https://x.example/ab')}</p>`)
  caretAtEnd(editor)
  backspace(editor)
  expect(editor.getHTML()).toBe(`<p>${a('https://x.example/a')}</p>`)
  type(editor, '?q=1')
  expect(editor.getHTML()).toBe(`<p>${a('https://x.example/a?q=1', 'https://x.example/a?q=1')}</p>`)
})

it('keeps whitespace and sentence punctuation after a URL out of it', () => {
  const editor = editorWith(`<p>(see ${a('https://x.example/ab', 'https://x.example/ab')}</p>`)
  caretAtEnd(editor)
  backspace(editor)
  type(editor, '), thanks.')
  expect(editor.getHTML()).toBe(`<p>(see ${a('https://x.example/a')}), thanks.</p>`)
})

it('keeps a URL link whole while a dot is typed into it', () => {
  const editor = editorWith(`<p>${a('https://x.example/a')}</p>`)
  caretAtEnd(editor)
  type(editor, '.html')
  expect(editor.getHTML()).toBe(`<p>${a('https://x.example/a.html', 'https://x.example/a.html')}</p>`)
})

it('splits a URL link where a space is typed into it', () => {
  const editor = editorWith(`<p>${a('https://x.example/a')}</p>`)
  editor.commands.setTextSelection(18)
  type(editor, ' ')
  expect(editor.getHTML()).toBe(`<p>${a('https://x.example', 'https://x.example')} /a</p>`)
})

it('leaves a label link with a URL elsewhere alone when edited', () => {
  const editor = editorWith(`<p>${a('Docs page', 'https://x.example/docs')}</p>`)
  editor.commands.setTextSelection(5)
  backspace(editor)
  expect(editor.getHTML()).toBe(`<p>${a('Doc page', 'https://x.example/docs')}</p>`)
})

it('keeps a replacement inside a line-opening link linked', () => {
  const editor = editorWith(`<p>${a('tset link')}</p>`)
  editor.commands.setTextSelection(1)
  // A spelling fix replaces the misspelt word while the caret stays put.
  editor.view.dispatch(editor.state.tr.insertText('test', 1, 5))
  expect(editor.getHTML()).toBe(`<p>${a('test link')}</p>`)
})

it('keeps a link stored for typing after its first character is deleted', () => {
  const editor = editorWith(`<p>Go ${a('link')}</p>`)
  const link = editor.state.doc.resolve(5).marks()
  editor.commands.setTextSelection(5)
  // A native backspace deletes the character and stores the marks it had.
  editor.view.dispatch(editor.state.tr.delete(4, 5).ensureMarks(link))
  type(editor, 'L')
  expect(editor.getHTML()).toBe(`<p>Go ${a('Link')}</p>`)
})

it('brings a deleted first character back linked on undo', () => {
  const editor = editorWith(`<p>${a('link')}</p>`)
  editor.view.dispatch(editor.state.tr.delete(1, 2))
  editor.commands.undo()
  expect(editor.getHTML()).toBe(`<p>${a('link')}</p>`)
})

it('keeps the links of a signature the app puts back above a link', () => {
  const editor = editorWith(`<p>Hi</p><p>${a('link')}</p>`)
  replaceEditorContentDiff(editor, `<p>Hi</p><p>${a('Me')}</p><p>${a('link')}</p>`)
  expect(editor.getHTML()).toBe(`<p>Hi</p><p>${a('Me')}</p><p>${a('link')}</p>`)
})

it('keeps a destination chosen for a URL-shaped label', () => {
  const editor = editorWith(`<p>${a('https://x.example/a')}</p>`)
  editor.chain().setTextSelection({ from: 1, to: 20 }).setLink({ href: 'https://y.example/b' }).run()
  expect(editor.getHTML()).toBe(`<p>${a('https://x.example/a', 'https://y.example/b')}</p>`)
  caretAtEnd(editor)
  backspace(editor)
  expect(editor.getHTML()).toBe(`<p>${a('https://x.example/', 'https://y.example/b')}</p>`)
})

it('keeps the text after an inserted URL out of it', () => {
  const editor = editorWith('<p>draft</p>')
  editor.commands.setTextSelection(1)
  const href = 'https://docs.example/'
  editor.commands.insertContent({ type: 'text', text: href, marks: [{ type: 'link', attrs: { href } }] })
  expect(editor.getHTML()).toBe(`<p>${a(href, href)}draft</p>`)
})

it('unlinks a URL an edit breaks rather than keep its old address', () => {
  const editor = editorWith(`<p>${a('https://x.example/a')}</p>`)
  editor.view.dispatch(editor.state.tr.delete(1, 2))
  expect(editor.getHTML()).toBe('<p>ttps://x.example/a</p>')
})

it('leaves a URL too long to read whole alone', () => {
  const href = `https://x.example/${'a'.repeat(5000)}`
  const editor = editorWith(`<p>${a(href, href)}</p>`)
  editor.commands.setTextSelection(30)
  type(editor, 'b')
  expect(editor.getHTML()).toBe(`<p>${a(href.slice(0, 29) + 'b' + href.slice(29), href)}</p>`)
})

it('finds the link in a selection for the Link dialog', () => {
  const editor = editorWith(`<p>See ${a('Docs', 'https://x.example/docs')}</p>`)
  editor.commands.setTextSelection({ from: 5, to: 9 })
  expect(linkAtCaret(editor as never)?.attrs.href).toBe('https://x.example/docs')
  caretAtEnd(editor)
  expect(linkAtCaret(editor as never)?.attrs.href).toBe('https://x.example/docs')
  editor.commands.setTextSelection(2)
  expect(linkAtCaret(editor as never)).toBeUndefined()
})

it('stops a URL link at a different link right after it', () => {
  const editor = editorWith(`<p>${a('https://x.example/a')}</p>`)
  caretAtEnd(editor)
  editor.commands.insertContent({
    type: 'text',
    text: 'Docs',
    marks: [{ type: 'link', attrs: { href: 'https://y.example/docs' } }],
  })
  type(editor, ' ')
  expect(editor.getHTML()).toBe(`<p>${a('https://x.example/a')}${a('Docs', 'https://y.example/docs')} </p>`)
})

it('leaves a bare domain link alone when text is typed after it', () => {
  const editor = editorWith(`<p>${a('example.com', 'http://example.com')}</p>`)
  caretAtEnd(editor)
  type(editor, ' ok')
  expect(editor.getHTML()).toBe(`<p>${a('example.com', 'http://example.com')} ok</p>`)
})

it('leaves a URL link alone when only the formatting after it changes', () => {
  const editor = editorWith(`<p>${a('https://x.example/a')}text</p>`)
  editor.chain().setTextSelection({ from: 20, to: 24 }).toggleBold().run()
  expect(editor.getHTML()).toBe(`<p>${a('https://x.example/a')}<strong>text</strong></p>`)
})

it('keeps the scheme a www link had', () => {
  const editor = editorWith(`<p>${a('www.x.example/ab', 'http://www.x.example/ab')}</p>`)
  caretAtEnd(editor)
  backspace(editor)
  expect(editor.getHTML()).toBe(`<p>${a('www.x.example/a', 'http://www.x.example/a')}</p>`)
})

it('finds the link a line opens with the caret before it for the Link dialog', () => {
  const editor = editorWith(`<p>${a('Docs', 'https://x.example/docs')}</p>`)
  editor.commands.setTextSelection(1)
  expect(linkAtCaret(editor as never)?.attrs.href).toBe('https://x.example/docs')
})

it('keeps an edit to the text glued after a URL from pulling that text in', () => {
  const url = 'https://x.example/r'
  const editor = editorWith(`<p>${a(url, url)}.Thanks</p>`)
  const h = editor.state.doc.textContent.indexOf('h', 20) + 1
  editor.view.dispatch(editor.state.tr.delete(h, h + 1))
  expect(editor.getHTML()).toBe(`<p>${a(url, url)}.Tanks</p>`)
})

it('syncs a paste of many links in one paragraph in linear time', () => {
  const editor = editorWith('<p></p>')
  const links = Array.from({ length: 10_000 }, (_, i) => a(`https://x.example/${i}`, `https://x.example/${i}`)).join('')
  const started = performance.now()
  editor.commands.insertContent(`<p>${links}</p>`)
  expect(performance.now() - started).toBeLessThan(1500)
  expect(editor.state.doc.firstChild?.childCount).toBe(10_000)
})

it('keeps the punctuation a URL link was written with when a space follows it', () => {
  const url = 'https://x.example/search?q=hi!'
  const editor = editorWith(`<p>${a(url, url)}</p>`)
  caretAtEnd(editor)
  type(editor, ' ok')
  expect(editor.getHTML()).toBe(`<p>${a(url, url)} ok</p>`)
})

it('keeps a label link pasted over a URL to the same address', () => {
  const url = 'https://x.example/a'
  const editor = editorWith(`<p>${a(url, url)}</p>`)
  editor
    .chain()
    .setTextSelection({ from: 1, to: 20 })
    .insertContent(`${a('Read docs', url)}`)
    .run()
  expect(editor.getHTML()).toBe(`<p>${a('Read docs', url)}</p>`)
})

it('syncs a paste of many links into a link-heavy paragraph in linear time', () => {
  const many = (prefix: string) =>
    Array.from({ length: 5_000 }, (_, i) => a(`https://${prefix}.example/${i}`, `https://${prefix}.example/${i}`)).join(
      '',
    )
  const editor = editorWith(`<p>${many('x')}</p>`)
  editor.commands.setTextSelection(3)
  const started = performance.now()
  editor.commands.insertContent(many('y'))
  expect(performance.now() - started).toBeLessThan(1500)
})

it('keeps a label linked when only its end is typed over', () => {
  const editor = editorWith(`<p>See ${a('Docx', 'https://x.example/docs')}</p>`)
  editor.commands.setTextSelection({ from: 8, to: 9 })
  type(editor, 's')
  expect(editor.getHTML()).toBe(`<p>See ${a('Docs', 'https://x.example/docs')}</p>`)
  // Typed over whole, the link goes, as in Gmail.
  editor.commands.setTextSelection({ from: 5, to: 9 })
  type(editor, 'Guide')
  expect(editor.getHTML()).toBe('<p>See Guide</p>')
})

it('resyncs a URL link whose scheme is typed over', () => {
  const url = 'https://x.example/a'
  const editor = editorWith(`<p>${a(url, url)}</p>`)
  editor.commands.setTextSelection({ from: 1, to: 9 })
  editor.view.dispatch(editor.state.tr.insertText('http://'))
  expect(editor.getHTML()).toBe(`<p>${a('http://x.example/a', 'http://x.example/a')}</p>`)
})

it('stops a URL link at inline code after it', () => {
  const url = 'https://x.example/a'
  const editor = editorWith(`<p>${a(url, url)}</p>`)
  caretAtEnd(editor)
  editor.commands.insertContent('<code>b</code>')
  expect(editor.getHTML()).toBe(`<p>${a(url, url)}<code>b</code></p>`)
})
