import { useEffect, useRef, useState } from 'react'
import { PenLine } from 'lucide-react'
import { EditorContent, useEditor } from '@tiptap/react'
import { StarterKit } from '@tiptap/starter-kit'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { settings$ } from '../../states/settings'
import { accounts$, setAccountSignature } from '../../states/accounts'
import { accountSignaturePayload, isBlankSignature } from '../../lib/signature'
import type { Account, AccountSignature } from '../../types'
import { ComposerToolbar } from '../composer/ComposerToolbar'
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

  const editor = useEditor({
    extensions: [StarterKit.configure({ link: { openOnClick: false } })],
    content: value,
    editorProps: {
      attributes: {
        class: 'tiptap-body focus:outline-none min-h-[110px] px-3.5 py-2.5 text-[0.8125rem] leading-relaxed',
        spellcheck: String(spellCheck),
      },
    },
    onUpdate: ({ editor }) => {
      clearTimeout(saveTimer.current)
      const html = editor.getHTML()
      saveTimer.current = setTimeout(
        () => onChangeRef.current(isBlankSignature(html) ? '' : html, ownerRef),
        SAVE_DEBOUNCE_MS,
      )
    },
  })

  // Flush a pending edit rather than dropping it when the editor unmounts (the
  // settings dialog closing, another account selected, the mode switched away
  // from Custom).
  useEffect(() => {
    return () => {
      if (!saveTimer.current) return
      clearTimeout(saveTimer.current)
      const html = editor?.getHTML() ?? ''
      onChangeRef.current(isBlankSignature(html) ? '' : html, ownerRef)
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

  if (!editor) return null

  return (
    <div>
      <ComposerToolbar editor={editor} onSetLink={setLink} />
      <EditorContent editor={editor} aria-label={t('settings.signature.label')} />
    </div>
  )
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
