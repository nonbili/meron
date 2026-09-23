import { useEffect, useRef, useState } from 'react'
import { CodeXml, PenLine } from 'lucide-react'
import { EditorContent, useEditor } from '@tiptap/react'
import { StarterKit } from '@tiptap/starter-kit'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { settings$ } from '../../states/settings'
import { accounts$, setAccountSignature } from '../../states/accounts'
import { accountSignaturePayload, savedSignatureHtml, unsupportedSignatureMarkup } from '../../lib/signature'
import type { Account, AccountSignature } from '../../types'
import { ComposerToolbar, ToolbarButton } from '../composer/ComposerToolbar'
import { ResizableImage } from '../composer/composerImage'
import { extractClipboardImages } from '../composer/composerHelpers'
import { SelectRow, SettingsGroup } from './AccountSettingsRows'

// Keystrokes shouldn't each cost a DB write (app-wide) or a bridge round trip
// (per account), so edits settle before they persist.
const SAVE_DEBOUNCE_MS = 600

type SignatureState = { mode: AccountSignature['mode']; html: string }

function signatureState(account: Account): SignatureState {
  return { mode: account.signature?.mode ?? 'global', html: account.signature?.html ?? '' }
}

/**
 * The rich-text editor behind both signature cards.
 *
 * Seeded once, from the `value` it mounts with: a save echoing back through
 * state must not yank the caret. Switching to another account remounts it (the
 * caller keys it by account id), and every save it reports carries the `owner`
 * it mounted with — the parent has already re-rendered for the new account by
 * the time this one's pending edit is flushed on unmount, so the text can only
 * be filed correctly if the editor says whose it is.
 */
function SignatureEditor({
  owner,
  value,
  onChange,
}: {
  owner: string
  value: string
  onChange: (html: string, owner: string) => void
}) {
  const { t } = useTranslation()
  const spellCheck = useValue(settings$.spellCheck)
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange

  // Captured at mount: this editor edits this subject's signature for its whole
  // life, whatever the parent has moved on to.
  const ownerRef = useRef(owner).current

  // Image files pasted or dropped in go in as data URLs: the composer's media
  // files are scratch copies, and a signature outlives any one draft. The
  // composer turns them into inline attachments when the mail is sent.
  //
  // `at` is where they go; without one they replace the selection, as any
  // paste does. All are read before any is inserted, so they land together
  // and in order rather than each at wherever the previous one left the caret.
  const insertImageFiles = async (files: File[], at?: number) => {
    const sources = await Promise.all(files.map(readAsDataUrl))
    if (!editor || editor.isDestroyed) return
    const images = sources.map((src, i) => ({ type: 'image', attrs: { src, alt: files[i].name } }))
    const chain = editor.chain().focus()
    if (at === undefined) chain.insertContent(images).run()
    // Reading is quick, but the document may still have changed under it.
    else chain.insertContentAt(Math.min(at, editor.state.doc.content.size), images).run()
  }

  const editor = useEditor({
    // The same image node as the composer's, so a signature with a logo or an
    // animated GIF lands in a draft exactly as it was written here.
    extensions: [
      StarterKit.configure({ link: { openOnClick: false } }),
      ResizableImage.configure({ allowBase64: true }),
    ],
    content: value,
    editorProps: {
      attributes: {
        class: 'tiptap-body focus:outline-none min-h-[110px] px-3.5 py-2.5 text-[0.8125rem] leading-relaxed',
        spellcheck: String(spellCheck),
      },
      handlePaste: (_view, event) => {
        // An image copied from a page comes with markup pointing at where it is
        // hosted; keeping that link beats embedding a copy of the file.
        if (/<img[\s>]/i.test(event.clipboardData?.getData('text/html') ?? '')) return false
        const files = extractClipboardImages(event.clipboardData)
        if (files.length === 0) return false
        event.preventDefault()
        void insertImageFiles(files)
        return true
      },
      handleDrop: (view, event, _slice, moved) => {
        if (moved || !event.dataTransfer) return false
        const files = extractClipboardImages(event.dataTransfer)
        if (files.length === 0) return false
        event.preventDefault()
        // Where the files were dropped, not the caret: that may be a selection
        // somewhere else entirely, which inserting there would overwrite.
        const at = view.posAtCoords({ left: event.clientX, top: event.clientY })?.pos
        if (at === undefined) return true
        void insertImageFiles(files, at)
        return true
      },
    },
    onUpdate: ({ editor }) => {
      clearTimeout(saveTimer.current)
      const html = savedSignatureHtml(editor.getHTML())
      saveTimer.current = setTimeout(() => onChangeRef.current(html, ownerRef), SAVE_DEBOUNCE_MS)
    },
  })

  // Flush a pending edit rather than dropping it when the editor unmounts (the
  // settings dialog closing, another account selected, the mode switched away
  // from Custom).
  useEffect(() => {
    return () => {
      if (!saveTimer.current) return
      clearTimeout(saveTimer.current)
      onChangeRef.current(savedSignatureHtml(editor?.getHTML() ?? ''), ownerRef)
    }
  }, [editor, ownerRef])

  useEffect(() => {
    editor?.view.dom.setAttribute('spellcheck', String(spellCheck))
  }, [editor, spellCheck])

  const setLink = () => {
    if (!editor) return
    const prev = editor.getAttributes('link').href as string | undefined
    const url = window.prompt('Link URL', prev ?? 'https://')
    if (url === null) return
    if (url === '') {
      editor.chain().focus().extendMarkRange('link').unsetLink().run()
      return
    }
    editor.chain().focus().extendMarkRange('link').setLink({ href: url }).run()
  }

  // The HTML being edited as source, or null while editing rich text. What the
  // textarea holds is the user's own text, left as typed; the editor is kept in
  // step behind it and is what gets saved, so a signature goes out exactly as a
  // composer will render it, whichever way it was written.
  //
  // Those can differ — the editor has no tables, colours or inline styles — so
  // the editor stays on screen under the source as a preview of what is saved,
  // with what is being dropped named above it.
  const [source, setSource] = useState<string | null>(null)
  const editingSource = source !== null
  const unsupported = source && editor ? unsupportedSignatureMarkup(source, editor.schema) : []

  // Read-only while previewing, so the only way to change the signature is the
  // source. No update event: nothing has changed that needs saving.
  useEffect(() => {
    editor?.setEditable(!editingSource, false)
  }, [editor, editingSource])

  const toggleSource = () => {
    if (!editor) return
    if (source !== null) {
      setSource(null)
      editor.commands.focus()
      return
    }
    setSource(savedSignatureHtml(editor.getHTML()))
  }

  const editSource = (html: string) => {
    setSource(html)
    editor?.commands.setContent(html)
  }

  if (!editor) return null

  const sourceToggle = (
    <ToolbarButton active={source !== null} onClick={toggleSource} title={t('settings.signature.editHtml')}>
      <CodeXml size={15} />
    </ToolbarButton>
  )

  return (
    <div>
      {source === null ? (
        <ComposerToolbar editor={editor} onSetLink={setLink} trailing={sourceToggle} />
      ) : (
        <div className="flex shrink-0 items-center justify-end border-b border-border bg-header px-3 py-1.5 select-none">
          {sourceToggle}
        </div>
      )}
      {source !== null && (
        <textarea
          aria-label={t('settings.signature.label')}
          value={source}
          onChange={(event) => editSource(event.target.value)}
          spellCheck={false}
          autoFocus
          className="block min-h-[110px] w-full resize-y bg-transparent px-3.5 py-2.5 font-mono text-[0.75rem] leading-relaxed text-primary focus:outline-none"
        />
      )}
      {source !== null && unsupported.length > 0 && (
        <p className="border-t border-border px-3.5 py-2 text-[0.6875rem] leading-relaxed text-amber-700/90 dark:text-amber-300/90">
          {t('settings.signature.unsupportedHtml', { markup: unsupported.join(', ') })}
        </p>
      )}
      {source !== null && (
        <p className="border-t border-border px-3.5 pt-2 text-[0.6875rem] text-secondary">
          {t('settings.signature.htmlPreview')}
        </p>
      )}
      <EditorContent editor={editor} aria-label={t('settings.signature.label')} />
    </div>
  )
}

function readAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  })
}

/**
 * The app-wide signature. Inserted into new messages, forwards and replies
 * opened in the full composer, for every account that doesn't override it.
 */
export function SignatureSettingsSection() {
  const { t } = useTranslation()
  const signature = useValue(settings$.signature)

  return (
    <SettingsGroup title={t('settings.sections.signature')}>
      <SignatureEditor owner="app" value={signature} onChange={(html) => settings$.signature.set(html)} />
      <p className="px-3.5 py-2 text-[0.6875rem] text-secondary">{t('settings.signature.hint')}</p>
    </SettingsGroup>
  )
}

/**
 * Per-account override: follow the app-wide signature, send none, or write one
 * just for this account. The custom text is kept when the mode changes, so
 * flipping away and back doesn't lose it.
 */
export function AccountSignatureCard({ account }: { account: Account }) {
  const { t } = useTranslation()
  const [state, setState] = useState(() => signatureState(account))
  // Re-seed during the render that brings a new account in, not in an effect:
  // an effect leaves one render where this card still holds the previous
  // account's text, which a save landing in that window would write to the
  // account now selected.
  const [seededFor, setSeededFor] = useState(account.id)
  if (seededFor !== account.id) {
    setSeededFor(account.id)
    setState(signatureState(account))
  }
  const { mode, html } = state

  const save = async (nextMode: AccountSignature['mode'], nextHtml: string) => {
    setState({ mode: nextMode, html: nextHtml })
    const stored = await setAccountSignature(account.id, accountSignaturePayload(nextMode, nextHtml))
    // A rejected write is rolled back in accounts state; this card holds its own
    // copy, so without this it would keep showing a choice that never persisted.
    if (stored) return
    const current = accounts$.peek().find((acc) => acc.id === accountRef.current)
    if (current && current.id === accountRef.current) setState(signatureState(current))
  }

  // Text flushed by an editor is filed against the account that editor was
  // opened for. Two cases, and they need different modes:
  //
  //   still this account  the mode as it stands now, so a flush arriving after
  //                       the user picked None or App signature does not put
  //                       Custom back (while a plain edit stays Custom).
  //   a past account      Custom — the mode it necessarily had, since an editor
  //                       only exists in Custom mode — because `mode` now
  //                       describes the account that replaced it.
  const currentRef = useRef({ id: account.id, mode })
  currentRef.current = { id: account.id, mode }
  const accountRef = useRef(account.id)
  accountRef.current = account.id
  const saveEditorHtml = (nextHtml: string, owner: string) => {
    const current = currentRef.current
    if (owner !== current.id) {
      void setAccountSignature(owner, accountSignaturePayload('custom', nextHtml))
      return
    }
    void save(current.mode, nextHtml)
  }

  return (
    <SettingsGroup title={t('settings.sections.signature')}>
      <SelectRow
        icon={<PenLine size={15} />}
        title={t('settings.signature.label')}
        hint={t('settings.signature.accountHint')}
        value={mode}
        options={[
          { value: 'global', label: t('settings.signature.modeGlobal') },
          { value: 'none', label: t('settings.signature.modeNone') },
          { value: 'custom', label: t('settings.signature.modeCustom') },
        ]}
        onChange={(next) => void save(next as AccountSignature['mode'], html)}
      />
      {mode === 'custom' && (
        <SignatureEditor key={account.id} owner={account.id} value={html} onChange={saveEditorHtml} />
      )}
    </SettingsGroup>
  )
}
