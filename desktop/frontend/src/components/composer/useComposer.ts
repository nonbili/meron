import { useEffect, useRef, useState } from 'react'
import { useEditor } from '@tiptap/react'
import { StarterKit } from '@tiptap/starter-kit'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { confirmAction, showToast } from '../../states/ui'
import { settings$ } from '../../states/settings'
import type { ComposeDraft, ComposerAttachment } from '../../types'
import {
  compose$,
  newDraftMessageId,
  sendComposed,
  appendSentMessage,
  saveComposedDraft,
  updateComposeDraft,
  finishClosingMessageTab,
} from '../../states/compose'
import { htmlToText } from '../../lib/html'
import { invoke } from '../../lib/bridge'
import { contextualErrorMessage } from '../../lib/errors'
import { discardSavedDraftCopy } from '../../states/mail'
import { pickFiles, pickImageFiles } from '../../lib/nativeFilePicker'
import { getComposeSession, registerComposeSession } from '../../states/composeSessions'
import { accounts$, isSendableAccount } from '../../states/accounts'
import { ResizableImage } from './composerImage'
import {
  clipboardHasImageMarkup,
  createInlineId,
  extractClipboardImages,
  inlineRichStyles,
  prepareInlineImages,
  readClipboardImages,
  readNativeClipboardImage,
  textToHtml,
  type NativeClipboardImage,
} from './composerHelpers'

// All of the Composer's behaviour: the tiptap editor, attachment handling
// (paste/drop/file-picker, inline images), rich/plain toggling, autosave, and
// send. The component consumes the returned editor, draft and handlers and is
// left as mostly markup.
export function useComposer(tabId: string) {
  const { t } = useTranslation()
  const tabs = useValue(compose$.tabs)
  const spellCheck = useValue(settings$.spellCheck)
  const tab = tabs.find((t) => t.id === tabId)
  const draft = tab?.compose
  const sessionRef = useRef<ReturnType<typeof getComposeSession> | null>(null)
  if (!sessionRef.current) sessionRef.current = getComposeSession(tabId)
  const session = sessionRef.current

  const [sending, setSending] = useState(false)
  const [error, setError] = useState('')
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle')

  /**
   * Stop autosaving and wait for whatever is already running. Every exit from
   * the composer goes through this first: a save in flight may be allocating
   * the very draft the caller is about to send or delete, and one queued behind
   * it would recreate the draft moments later.
   */
  const stopSaving = async () => {
    session.savesStopped = true
    await session.saveChain
  }

  /** The draft as the tab holds it now, rather than as this render saw it. */
  const latestDraft = () => compose$.tabs.peek().find((t) => t.id === tabId)?.compose

  /**
   * Delete the server-side copy of a draft: the one the last save actually
   * wrote, which by then is the allocated id rather than the local placeholder
   * the composer opened with.
   *
   * A tab opened from the Drafts list carries `sourceDraft`, whose thread is
   * the draft itself and disappears with it. A reply escalated out of a
   * conversation has no such row — only the tab's conversation, which keeps its
   * card and needs its Draft badge cleared once the draft behind it is gone.
   */
  const discardRemoteDraft = async (target: ComposeDraft, throwOnError = false) => {
    const remoteId = target.draftMessageId?.startsWith('local-draft-') ? undefined : target.draftMessageId
    if (!remoteId && !target.sourceDraft) return
    await discardSavedDraftCopy(
      {
        threadId: target.sourceDraft?.threadId ?? '',
        messageId: target.sourceDraft?.messageId ?? '',
        folderId: target.sourceDraft?.folderId ?? '',
        accountId: target.accountId,
        draftMessageId: remoteId,
        replyThreadId: target.sourceDraft ? '' : (tab?.threadId ?? ''),
      },
      { throwOnError },
    )
  }

  /**
   * Throw the draft away: stop saving, let the queue finish so nothing is left
   * mid-allocation, delete the server copy, then close. Closing on its own is
   * not enough — a save still running would put the draft back seconds later,
   * with nothing left on screen to explain where it came from.
   */
  const discardAndClose = async () => {
    setError('')
    try {
      await stopSaving()
      const current = latestDraft()
      if (current) await discardRemoteDraft(current, true)
      finishClosingMessageTab(tabId)
    } catch (err) {
      const message = contextualErrorMessage(err, t('composer.status.couldNotDiscardDraft'))
      showToast(message, 'error')
      finishClosingMessageTab(tabId)
    }
  }
  const [saveError, setSaveError] = useState('')
  const lastImagePasteAtRef = useRef(0)
  const previousFromEmailRef = useRef(draft?.fromEmail)
  // Captured once so the editor isn't reset on every keystroke / re-render.
  const initialHtml = useRef(draft?.html ?? '').current
  // When the composer opens with recipients already filled (reply/forward/
  // mailto), drop the cursor straight into the body; only a blank new message
  // should land on the To field.
  const focusBody = useRef((draft?.to ?? '').trim().length > 0).current
  // A seeded signature sits at the top of the body, above any quote, so landing
  // at the end would drop the cursor below it — and below the quote. Start puts
  // it on the blank line the signature was inserted under.
  const focusAt = useRef(draft?.signature?.html ? ('start' as const) : ('end' as const)).current

  const editor = useEditor({
    extensions: [
      StarterKit.configure({ link: { openOnClick: false } }),
      ResizableImage.configure({
        allowBase64: true,
        HTMLAttributes: {
          class: 'my-2 max-w-full rounded-lg',
        },
      }),
    ],
    content: initialHtml,
    editorProps: {
      attributes: {
        class: 'tiptap-body focus:outline-none min-h-[240px] text-[0.875rem] leading-relaxed',
        spellcheck: String(spellCheck),
      },
      handlePaste: (_view, event) => {
        const imageFiles = extractClipboardImages(event.clipboardData)
        if (imageFiles.length === 0) {
          if (!clipboardHasImageMarkup(event.clipboardData)) return false
          event.preventDefault()
          pasteImageFromClipboard(Date.now(), { inline: true })
          return true
        }
        event.preventDefault()
        lastImagePasteAtRef.current = Date.now()
        addFiles(imageFiles, { inline: true })
        return true
      },
      handleDrop: (_view, event, _slice, moved) => {
        if (!moved && event.dataTransfer) {
          const imageFiles = extractClipboardImages(event.dataTransfer)
          if (imageFiles.length > 0) {
            event.preventDefault()
            addFiles(imageFiles, { inline: true })
            return true
          }
        }
        return false
      },
    },
    autofocus: focusBody ? focusAt : false,
    onUpdate: ({ editor }) => updateComposeDraft(tabId, { html: editor.getHTML() }),
  })

  useEffect(() => {
    if (draft) registerComposeSession(tabId, discardAndClose)
  }, [draft, session, tabId])

  useEffect(() => {
    editor?.view.dom.setAttribute('spellcheck', String(spellCheck))
  }, [editor, spellCheck])

  // The plaintext body has the same problem, and an autofocused <textarea>
  // always lands at the end: put the caret just before the signature instead,
  // which is where the message being written belongs.
  const textRef = useRef<HTMLTextAreaElement | null>(null)
  useEffect(() => {
    const el = textRef.current
    if (!el || !focusBody) return
    const signature = draft?.signature?.text
    if (!signature) return
    const at = el.value.indexOf(signature)
    if (at < 0) return
    const caret = el.value.slice(0, at).replace(/\s+$/, '').length
    el.setSelectionRange(caret, caret)
    // Mount only: after that the caret belongs to the user.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Changing the From account rewrites the body's signature in state (see
  // states/compose). The editor holds its own copy of the document, so it has
  // to be re-seeded — the only edit to the body that doesn't come from typing.
  const editorAccountRef = useRef(draft?.accountId)
  useEffect(() => {
    if (!editor || !draft?.rich) return
    if (editorAccountRef.current === draft.accountId) return
    editorAccountRef.current = draft.accountId
    if (draft.html === editor.getHTML()) return
    const { from, to } = editor.state.selection
    editor.commands.setContent(draft.html || '<p></p>', { emitUpdate: false })
    // Keep the caret where the user was typing when the new document is long
    // enough to still hold it; otherwise tiptap leaves it at the start.
    if (to <= editor.state.doc.content.size) editor.commands.setTextSelection({ from, to })
  }, [editor, draft?.accountId, draft?.rich, draft?.html])

  // A draft that moves to another account leaves its Drafts copy behind: the
  // allocated id belongs to the account whose folder holds it, and saving under
  // the new account creates a second copy while the first lingers. Queue the
  // cleanup on the save chain so it cannot overtake a save still writing it.
  useEffect(() => {
    const previous = { accountId: session.savedAccountId, draftMessageId: session.savedDraftMessageId }
    const accountId = draft?.accountId
    if (!accountId || previous.accountId === accountId) {
      if (accountId) {
        session.savedAccountId = accountId
        session.savedDraftMessageId = draft?.draftMessageId
      }
      return
    }
    session.savedAccountId = accountId
    session.savedDraftMessageId = draft?.draftMessageId
    const orphan = previous.draftMessageId
    if (!previous.accountId || !orphan || orphan.startsWith('local-draft-')) return

    // The next save must not reuse the old account's id, so the tab goes back
    // to a placeholder and the old copy is deleted where it actually lives.
    updateComposeDraft(tabId, { draftMessageId: newDraftMessageId() })
    session.savedDraftMessageId = undefined
    session.saveChain = session.saveChain.then(async () => {
      try {
        await discardSavedDraftCopy({
          threadId: '',
          messageId: '',
          folderId: '',
          accountId: previous.accountId,
          draftMessageId: orphan,
        })
      } catch (err) {
        console.error('Discarding the previous account’s draft failed:', err)
      }
    })
  }, [session, tabId, draft?.accountId, draft?.draftMessageId])

  useEffect(() => {
    const sendAsChanged = previousFromEmailRef.current !== draft?.fromEmail
    previousFromEmailRef.current = draft?.fromEmail
    if (!draft || !draft.accountId || sending) return

    setSaveStatus('idle')
    setSaveError('')
    const saveDraft = async () => {
      // The draft is read from the tab when the save actually runs, never from
      // the render that queued it. A queued save can wait behind a slower one
      // while the user changes the From account or keeps typing: pairing that
      // render's account and headers with the body as it stands now would file
      // one account's message under another's, and reusing that render's draft
      // id would allocate a second server draft.
      if (session.savesStopped) return
      const current = compose$.tabs.peek().find((t) => t.id === tabId)?.compose
      if (!current?.accountId) return

      // Autosaves overlap: a send-as change starts one immediately, and another
      // edit (a change of account, say) can start the next before it lands. Only
      // the newest may report back — an older one completing afterwards would
      // pin the tab to the draft id of a message it no longer describes.
      const generation = ++session.saveGeneration
      const isCurrent = () => generation === session.saveGeneration
      setSaveStatus('saving')
      setSaveError('')
      try {
        let content = current.rich ? current.html : current.text
        let attachments = current.attachments
        if (current.rich) {
          const prepared = await prepareInlineImages(content, attachments)
          content = inlineRichStyles(prepared.html)
          attachments = prepared.attachments
        }
        const draftMessageId = current.draftMessageId
        const savedDraftId = await saveComposedDraft({
          accountId: current.accountId,
          from: current.fromEmail,
          to: current.to.trim(),
          cc: current.cc.trim(),
          bcc: current.bcc.trim(),
          replyTo: current.replyTo.trim(),
          subject: current.subject.trim(),
          rich: current.rich,
          content,
          inReplyTo: current.inReplyTo,
          references: current.references,
          draftMessageId,
          attachments,
        })
        const latest = compose$.tabs.peek().find((t) => t.id === tabId)?.compose
        if (latest?.accountId !== current.accountId) {
          if (savedDraftId !== draftMessageId) {
            await discardSavedDraftCopy(
              { threadId: '', messageId: '', folderId: '', accountId: current.accountId, draftMessageId: savedDraftId },
              { failureMessage: "Couldn't clean up the previous account's draft" },
            )
          }
          if (isCurrent()) {
            setSaveStatus('idle')
            setSaveError('')
          }
          return
        }
        // Only the account that allocated an id may attach it to the tab.
        if (savedDraftId !== draftMessageId) updateComposeDraft(tabId, { draftMessageId: savedDraftId })
        if (!isCurrent()) return
        setSaveStatus('saved')
        setSaveError('')
      } catch (err) {
        console.error('Autosave draft failed:', err)
        if (!isCurrent()) return
        setSaveStatus('error')
        setSaveError(contextualErrorMessage(err, t('composer.status.draftAutosaveFailed')))
      }
    }

    // Queue behind whatever save is already running, so two never allocate or
    // write the same draft at once.
    const queueSave = () => {
      session.saveChain = session.saveChain.then(saveDraft, saveDraft)
    }

    // A send-as choice is often the last edit before the composer closes. Start
    // that save immediately so unmounting cannot cancel it with the debounce.
    if (sendAsChanged) {
      queueSave()
      return
    }

    const timer = setTimeout(queueSave, 3000)
    return () => clearTimeout(timer)
  }, [
    draft?.accountId,
    draft?.fromEmail,
    draft?.to,
    draft?.cc,
    draft?.bcc,
    draft?.subject,
    draft?.replyTo,
    draft?.rich,
    draft?.text,
    draft?.html,
    draft?.attachments,
    sending,
    editor,
    session,
  ])

  const update = (partial: Parameters<typeof updateComposeDraft>[1]) => updateComposeDraft(tabId, partial)

  // Toggling the body between rich and plaintext rewrites it through
  // htmlToText/textToHtml, so any signature inside it comes out transformed too.
  // The tracked copy has to go through the very same conversion or it could no
  // longer be found in the body it is supposed to describe (see lib/signature).
  const toggleRich = () => {
    if (!draft) return
    const mark = draft.signature
    if (draft.rich) {
      // Rich -> plaintext: capture text fallback.
      const html = editor?.getHTML() ?? draft.html
      const signature = mark ? { ...mark, text: htmlToText(mark.html).trim() } : mark
      update({ rich: false, html, text: htmlToText(html), signature })
    } else {
      // Plaintext -> rich: seed editor from the plaintext content.
      const html = textToHtml(draft.text)
      const signature = mark ? { ...mark, html: textToHtml(mark.text) } : mark
      editor?.commands.setContent(html || '<p></p>')
      update({ rich: true, html, signature })
    }
  }

  const addFiles = (files: ArrayLike<File>, options: { inline?: boolean } = {}) => {
    for (let i = 0; i < files.length; i++) {
      const file = files[i]
      const reader = new FileReader()
      reader.onload = () => {
        const dataUrl = reader.result as string
        const base64Data = dataUrl.split(',')[1]
        const ext = file.type.startsWith('image/') ? file.type.slice(6) : ''
        const filename = file.name || (ext ? `pasted-image-${Date.now()}.${ext}` : `pasted-${Date.now()}`)
        const shouldInline = !!options.inline && file.type.toLowerCase().startsWith('image/') && !!editor
        const next: ComposerAttachment = {
          id: `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`,
          filename,
          mime: file.type || 'application/octet-stream',
          size: file.size,
          data: base64Data,
          inlineId: shouldInline ? createInlineId() : undefined,
        }
        update({
          attachments: [...(compose$.tabs.get().find((t) => t.id === tabId)?.compose?.attachments ?? []), next],
        })
        if (shouldInline) {
          invoke<string>('composer.writeMediaFile', { data: base64Data, filename })
            .then((url) => {
              editor!.chain().focus().setImage({ src: url, alt: filename }).run()
            })
            .catch((err) => {
              console.error('Failed to write media file:', err)
              editor!.chain().focus().setImage({ src: dataUrl, alt: filename }).run()
            })
        }
      }
      reader.readAsDataURL(file)
    }
  }

  const addNativeClipboardImage = (image: NativeClipboardImage, options: { inline?: boolean } = {}) => {
    const shouldInline = !!options.inline && image.mime.toLowerCase().startsWith('image/') && !!editor
    const next: ComposerAttachment = {
      id: `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`,
      filename: image.filename,
      mime: image.mime || 'application/octet-stream',
      size: image.size,
      data: image.data,
      inlineId: shouldInline ? createInlineId() : undefined,
    }
    update({
      attachments: [...(compose$.tabs.get().find((t) => t.id === tabId)?.compose?.attachments ?? []), next],
    })
    if (shouldInline) {
      invoke<string>('composer.writeMediaFile', { data: image.data, filename: image.filename })
        .then((url) => {
          editor!.chain().focus().setImage({ src: url, alt: image.filename }).run()
        })
        .catch((err) => {
          console.error('Failed to write media file:', err)
          editor!
            .chain()
            .focus()
            .setImage({ src: `data:${image.mime || 'image/png'};base64,${image.data}`, alt: image.filename })
            .run()
        })
    }
  }

  const pickAttachmentFiles = async () => {
    try {
      const files = await pickFiles(t('composer.actions.attachFiles'))
      if (files.length > 0) addFiles(files)
    } catch (error) {
      showToast(error instanceof Error ? error.message : t('chat.failedToChooseFiles'), 'error')
    }
  }

  const pickInlineImages = async () => {
    try {
      const files = await pickImageFiles(t('composer.actions.insertInlineImages'))
      if (files.length > 0) addFiles(files, { inline: true })
    } catch (error) {
      showToast(error instanceof Error ? error.message : t('chat.failedToChooseImages'), 'error')
    }
  }

  const handlePaste = (e: React.ClipboardEvent) => {
    const imageFiles = extractClipboardImages(e.clipboardData)
    if (imageFiles.length > 0) {
      e.preventDefault()
      lastImagePasteAtRef.current = Date.now()
      addFiles(imageFiles)
    } else if (clipboardHasImageMarkup(e.clipboardData)) {
      e.preventDefault()
      pasteImageFromClipboard(Date.now(), { inline: false })
    }
  }

  // WebKitGTK often doesn't expose pasted images via the synchronous paste
  // event's clipboardData. As a fallback, intercept Ctrl/Cmd+V and read the
  // clipboard asynchronously.
  const handleKeyDown = async (e: React.KeyboardEvent) => {
    const isPaste = (e.ctrlKey || e.metaKey) && (e.key === 'v' || e.key === 'V')
    if (!isPaste) return

    pasteImageFromClipboard(Date.now(), { inline: !!draft?.rich })
  }

  const pasteImageFromClipboard = (pasteStartedAt: number, options: { inline: boolean }) => {
    void readClipboardImages()
      .then((images) => {
        if (lastImagePasteAtRef.current >= pasteStartedAt) return
        if (images.length === 0) {
          return readNativeClipboardImage().then((image) => {
            if (!image || lastImagePasteAtRef.current >= pasteStartedAt) return
            lastImagePasteAtRef.current = Date.now()
            addNativeClipboardImage(image, options)
          })
        }
        lastImagePasteAtRef.current = Date.now()
        addFiles(images, options)
      })
      .catch(() => {
        void readNativeClipboardImage()
          .then((image) => {
            if (!image || lastImagePasteAtRef.current >= pasteStartedAt) return
            lastImagePasteAtRef.current = Date.now()
            addNativeClipboardImage(image, options)
          })
          .catch(() => undefined)
      })
  }

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

  const canSend = !sending && !!draft?.accountId && !!draft?.to.trim()

  const submit = async () => {
    if (!canSend || !draft) return
    setSending(true)
    setError('')
    await stopSaving()
    // Re-read after the wait: the fields stay editable while a save drains, and
    // sending this render's copy would silently drop whatever was typed (or the
    // account picked) in the meantime.
    let current = latestDraft()
    if (!current) {
      setSending(false)
      return
    }
    const accountId = current.accountId
    const account = accounts$.peek().find((candidate) => candidate.id === accountId)
    if (!current.to.trim() || !isSendableAccount(account)) {
      session.savesStopped = false
      setError(
        !current.to.trim()
          ? t('composer.status.addRecipientBeforeSending')
          : t('composer.status.chooseAvailableAccount'),
      )
      setSending(false)
      return
    }
    let subject = current.subject.trim()
    if (
      !subject &&
      !(await confirmAction({
        title: 'No subject',
        message: 'Send this message without a subject?',
        confirmLabel: 'Send',
      }))
    ) {
      session.savesStopped = false
      setSending(false)
      return
    }
    // The confirmation itself is another await while fields remain editable.
    // Re-read once more so confirming never sends stale recipients or content.
    current = latestDraft()
    const confirmedAccount = accounts$.peek().find((candidate) => candidate.id === current?.accountId)
    if (!current?.to.trim() || !isSendableAccount(confirmedAccount)) {
      session.savesStopped = false
      setError(
        !current?.to.trim()
          ? t('composer.status.addRecipientBeforeSending')
          : t('composer.status.chooseAvailableAccount'),
      )
      setSending(false)
      return
    }
    subject = current.subject.trim()
    try {
      let content = current.rich ? current.html : current.text
      let attachments = current.attachments
      if (current.rich) {
        const prepared = await prepareInlineImages(content, attachments)
        content = inlineRichStyles(prepared.html)
        attachments = prepared.attachments
      }
      await sendComposed({
        accountId: current.accountId,
        from: current.fromEmail,
        to: current.to.trim(),
        cc: current.cc.trim(),
        bcc: current.bcc.trim(),
        replyTo: current.replyTo.trim(),
        subject,
        rich: current.rich,
        content,
        inReplyTo: current.inReplyTo,
        references: current.references,
        attachments,
      })
      // When this tab is a reply to the open conversation, drop the sent message
      // into the thread immediately so it shows without waiting for the next sync.
      if (tab?.threadId) {
        appendSentMessage({
          threadId: tab.threadId,
          accountId: current.accountId,
          from: current.fromEmail,
          to: current.to.trim(),
          cc: current.cc.trim(),
          bcc: current.bcc.trim(),
          subject,
          rich: current.rich,
          content,
          references: current.references,
          attachments,
        })
      }
      await discardRemoteDraft(current)
      showToast(t('chat.messageSent'))
      finishClosingMessageTab(tabId)
    } catch (err) {
      // The composer stays open on failure, so autosaving resumes with it.
      session.savesStopped = false
      setError(contextualErrorMessage(err, t('compose.toast.sendFailed')))
    } finally {
      setSending(false)
    }
  }

  return {
    tab,
    draft,
    editor,
    focusBody,
    textRef,
    sending,
    error,
    saveStatus,
    saveError,
    canSend,
    update,
    toggleRich,
    addFiles,
    pickAttachmentFiles,
    pickInlineImages,
    handlePaste,
    handleKeyDown,
    setLink,
    submit,
  }
}
