import { useEffect, useRef, useState } from 'react'
import { useEditor } from '@tiptap/react'
import { StarterKit } from '@tiptap/starter-kit'
import { useValue } from '@legendapp/state/react'
import { confirmAction, showToast } from '../../states/ui'
import { settings$ } from '../../states/settings'
import type { ComposerAttachment } from '../../types'
import {
  compose$,
  sendComposed,
  appendSentMessage,
  saveComposedDraft,
  updateComposeDraft,
  closeMessageTab,
} from '../../states/compose'
import { htmlToText } from '../../lib/html'
import { invoke } from '../../lib/bridge'
import { contextualErrorMessage } from '../../lib/errors'
import { discardSavedDraftCopy } from '../../states/mail'
import { pickFiles, pickImageFiles } from '../../lib/nativeFilePicker'
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
  const tabs = useValue(compose$.tabs)
  const spellCheck = useValue(settings$.spellCheck)
  const tab = tabs.find((t) => t.id === tabId)
  const draft = tab?.compose

  const [sending, setSending] = useState(false)
  const [error, setError] = useState('')
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle')
  const [saveError, setSaveError] = useState('')
  const lastImagePasteAtRef = useRef(0)
  // Captured once so the editor isn't reset on every keystroke / re-render.
  const initialHtml = useRef(draft?.html ?? '').current
  // When the composer opens with recipients already filled (reply/forward/
  // mailto), drop the cursor straight into the body; only a blank new message
  // should land on the To field.
  const focusBody = useRef((draft?.to ?? '').trim().length > 0).current

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
        class: 'tiptap-body focus:outline-none min-h-[240px] text-[14px] leading-relaxed',
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
    autofocus: focusBody ? 'end' : false,
    onUpdate: ({ editor }) => updateComposeDraft(tabId, { html: editor.getHTML() }),
  })

  useEffect(() => {
    editor?.view.dom.setAttribute('spellcheck', String(spellCheck))
  }, [editor, spellCheck])

  useEffect(() => {
    if (!draft || !draft.accountId || sending) return

    setSaveStatus('idle')
    setSaveError('')
    const timer = setTimeout(async () => {
      setSaveStatus('saving')
      setSaveError('')
      try {
        let content = draft.rich ? (editor?.getHTML() ?? draft.html) : draft.text
        let attachments = draft.attachments
        if (draft.rich) {
          const prepared = await prepareInlineImages(content, attachments)
          content = inlineRichStyles(prepared.html)
          attachments = prepared.attachments
        }
        const savedDraftId = await saveComposedDraft({
          accountId: draft.accountId,
          from: draft.fromEmail,
          to: draft.to.trim(),
          cc: draft.cc.trim(),
          bcc: draft.bcc.trim(),
          replyTo: draft.replyTo.trim(),
          subject: draft.subject.trim(),
          rich: draft.rich,
          content,
          inReplyTo: draft.inReplyTo,
          references: draft.references,
          draftMessageId: draft.draftMessageId,
          attachments,
        })
        if (savedDraftId !== draft.draftMessageId) updateComposeDraft(tabId, { draftMessageId: savedDraftId })
        setSaveStatus('saved')
        setSaveError('')
      } catch (err) {
        console.error('Autosave draft failed:', err)
        setSaveStatus('error')
        setSaveError(contextualErrorMessage(err, 'Draft autosave failed'))
      }
    }, 3000)

    return () => clearTimeout(timer)
  }, [
    draft?.accountId,
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
  ])

  const update = (partial: Parameters<typeof updateComposeDraft>[1]) => updateComposeDraft(tabId, partial)

  const toggleRich = () => {
    if (!draft) return
    if (draft.rich) {
      // Rich -> plaintext: capture text fallback.
      const html = editor?.getHTML() ?? draft.html
      update({ rich: false, html, text: htmlToText(html) })
    } else {
      // Plaintext -> rich: seed editor from the plaintext content.
      const html = textToHtml(draft.text)
      editor?.commands.setContent(html || '<p></p>')
      update({ rich: true, html })
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
      const files = await pickFiles('Attach files')
      if (files.length > 0) addFiles(files)
    } catch (error) {
      showToast(error instanceof Error ? error.message : 'Failed to choose files', 'error')
    }
  }

  const pickInlineImages = async () => {
    try {
      const files = await pickImageFiles('Insert inline images')
      if (files.length > 0) addFiles(files, { inline: true })
    } catch (error) {
      showToast(error instanceof Error ? error.message : 'Failed to choose images', 'error')
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
    const subject = draft.subject.trim()
    if (
      !subject &&
      !(await confirmAction({
        title: 'No subject',
        message: 'Send this message without a subject?',
        confirmLabel: 'Send',
      }))
    ) {
      return
    }

    setSending(true)
    setError('')
    try {
      let content = draft.rich ? (editor?.getHTML() ?? draft.html) : draft.text
      let attachments = draft.attachments
      if (draft.rich) {
        const prepared = await prepareInlineImages(content, attachments)
        content = inlineRichStyles(prepared.html)
        attachments = prepared.attachments
      }
      await sendComposed({
        accountId: draft.accountId,
        from: draft.fromEmail,
        to: draft.to.trim(),
        cc: draft.cc.trim(),
        bcc: draft.bcc.trim(),
        replyTo: draft.replyTo.trim(),
        subject,
        rich: draft.rich,
        content,
        inReplyTo: draft.inReplyTo,
        references: draft.references,
        attachments,
      })
      // When this tab is a reply to the open conversation, drop the sent message
      // into the thread immediately so it shows without waiting for the next sync.
      if (tab?.threadId) {
        appendSentMessage({
          threadId: tab.threadId,
          accountId: draft.accountId,
          from: draft.fromEmail,
          to: draft.to.trim(),
          cc: draft.cc.trim(),
          bcc: draft.bcc.trim(),
          subject,
          rich: draft.rich,
          content,
          references: draft.references,
          attachments,
        })
      }
      if (draft.draftMessageId || draft.sourceDraft) {
        void discardSavedDraftCopy({
          threadId: draft.sourceDraft?.threadId ?? '',
          messageId: draft.sourceDraft?.messageId ?? '',
          folderId: draft.sourceDraft?.folderId ?? '',
          accountId: draft.accountId,
          draftMessageId: draft.draftMessageId,
        })
      }
      showToast('Sent')
      closeMessageTab(tabId)
    } catch (err) {
      setError(contextualErrorMessage(err, 'Send failed'))
    } finally {
      setSending(false)
    }
  }

  return {
    tab,
    draft,
    editor,
    focusBody,
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
