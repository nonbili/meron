import { useCallback, useEffect, useRef, useState } from 'react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import {
  cancelQuickReplyDraftSave,
  compose$,
  discardQuickReplyDraftIfEmpty,
  isQuickReplyBlank,
  quickReplyCaretOffset,
  saveQuickReplyDraft,
  scheduleQuickReplyDraftSave,
  seedQuickReplySignature,
  sendReply,
} from '../../states/compose'
import { getActiveThread } from '../../states/mail'
import { showToast, ui$ } from '../../states/ui'
import { settings$, isSendKey } from '../../states/settings'
import { pickFiles } from '../../lib/nativeFilePicker'
import {
  extractClipboardImages,
  logComposerPaste,
  readClipboardImages,
  readNativeClipboardImage,
  type NativeClipboardImage,
} from './quickReplyClipboard'

const QUICK_REPLY_MAX_VISIBLE_LINES = 12
const QUICK_REPLY_LINE_HEIGHT_PX = 20
const QUICK_REPLY_VERTICAL_PADDING_PX = 14
const QUICK_REPLY_MAX_HEIGHT_PX =
  QUICK_REPLY_MAX_VISIBLE_LINES * QUICK_REPLY_LINE_HEIGHT_PX + QUICK_REPLY_VERTICAL_PADDING_PX

// State and behaviour for the quick-reply box: per-thread draft hydration and
// autosave, attachment handling (file picker + sync/async/native paste), the
// auto-growing textarea, the reply-focus shortcut and send. The component renders
// the returned values.
export function useQuickReply() {
  const { t } = useTranslation()
  const composer = useValue(compose$.composer)
  const composerAttachments = useValue(compose$.composerAttachments)
  const sendShortcut = useValue(settings$.sendShortcut)
  const activeThread = useValue(getActiveThread)
  const activeThreadId = activeThread?.thread_id ?? ''

  const textareaRef = useRef<HTMLTextAreaElement | null>(null)
  const replyFocus = useValue(ui$.replyFocus)
  const lastImagePasteAtRef = useRef(0)
  const lastHydratedThreadRef = useRef('')
  const suppressNextDraftSaveRef = useRef(false)
  const [sendingReply, setSendingReply] = useState(false)

  const handleSendReply = useCallback(async () => {
    if (sendingReply) return
    setSendingReply(true)
    try {
      await sendReply()
    } finally {
      setSendingReply(false)
    }
  }, [sendingReply])

  useEffect(() => {
    const textarea = textareaRef.current
    if (!textarea) return
    textarea.style.height = 'auto'
    textarea.style.height = `${Math.min(textarea.scrollHeight, QUICK_REPLY_MAX_HEIGHT_PX)}px`
  }, [composer])

  // Focus the box when the "r" shortcut fires (ignore the initial 0 value).
  // The caret goes to the end of the user's own text, not the end of the box —
  // the seeded signature sits below it, and typing into that is not what "r"
  // should hand you.
  useEffect(() => {
    if (replyFocus === 0) return
    const textarea = textareaRef.current
    textarea?.focus()
    const caret = quickReplyCaretOffset()
    textarea?.setSelectionRange(caret, caret)
  }, [replyFocus])

  // Reset quick reply state when the user switches threads. Any pending
  // debounced save for the outgoing thread is dropped rather than flushed
  // (mirrors the mobile app's behavior), and the incoming thread's saved draft
  // (if its tail message is one) is re-hydrated by hydrateQuickReplyFromTailDraft
  // once its messages load.
  useEffect(() => {
    const previous = lastHydratedThreadRef.current
    if (previous === activeThreadId) return
    cancelQuickReplyDraftSave()
    lastHydratedThreadRef.current = activeThreadId
    suppressNextDraftSaveRef.current = true
    compose$.quickReplyDraftId.set('')
    compose$.quickReplyDraftSaved.set(false)
    compose$.quickReplyFrom.set('')
    // Starts the new thread's box on the replying account's signature rather
    // than blank — the box is what gets sent, so it shows what will go out.
    seedQuickReplySignature()
  }, [activeThreadId])

  // Save or discard the current quick reply after text changes. Keyed on
  // `composer` only (not `activeThreadId`), and scheduled against
  // `lastHydratedThreadRef.current`, so thread-switch resets do not autosave.
  useEffect(() => {
    const owner = lastHydratedThreadRef.current
    if (!owner) return
    if (suppressNextDraftSaveRef.current) {
      suppressNextDraftSaveRef.current = false
      return
    }
    if (isQuickReplyBlank()) {
      cancelQuickReplyDraftSave()
      void discardQuickReplyDraftIfEmpty()
    } else {
      scheduleQuickReplyDraftSave()
    }
  }, [composer, composerAttachments.length])

  // Attachments are a discrete action rather than a keystroke stream, so save
  // (or discard, if that was the last thing keeping the draft non-empty)
  // immediately instead of waiting out the debounce.
  useEffect(() => {
    const owner = lastHydratedThreadRef.current
    if (!owner) return
    cancelQuickReplyDraftSave()
    if (isQuickReplyBlank()) {
      void discardQuickReplyDraftIfEmpty()
    } else {
      void saveQuickReplyDraft()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [composerAttachments])

  const addAttachmentFiles = (files: ArrayLike<File>) => {
    logComposerPaste(
      'adding attachments',
      Array.from(files).map((file) => ({
        name: file.name,
        type: file.type,
        size: file.size,
      })),
    )
    for (let i = 0; i < files.length; i++) {
      const file = files[i]
      const reader = new FileReader()
      reader.onload = () => {
        const result = reader.result as string
        const base64Data = result.split(',')[1]
        const ext = file.type.startsWith('image/') ? file.type.slice(6) : ''
        const filename = file.name || (ext ? `pasted-image-${Date.now()}.${ext}` : `pasted-${Date.now()}`)
        compose$.composerAttachments.push({
          id: `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`,
          filename,
          mime: file.type || 'application/octet-stream',
          size: file.size,
          data: base64Data,
        })
      }
      reader.readAsDataURL(file)
    }
  }

  const pickAttachmentFiles = async () => {
    try {
      const files = await pickFiles(t('composer.actions.attachFiles'))
      if (files.length > 0) addAttachmentFiles(files)
    } catch (error) {
      showToast(error instanceof Error ? error.message : t('chat.failedToChooseFiles'), 'error')
    }
  }

  const addNativeClipboardImage = (image: NativeClipboardImage) => {
    compose$.composerAttachments.push({
      id: `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`,
      filename: image.filename,
      mime: image.mime || 'application/octet-stream',
      size: image.size,
      data: image.data,
    })
  }

  const handleComposerPaste = (e: React.ClipboardEvent) => {
    logComposerPaste('paste event fired')
    const images = extractClipboardImages(e.clipboardData)
    if (images.length > 0) {
      e.preventDefault()
      lastImagePasteAtRef.current = Date.now()
      addAttachmentFiles(images)
    }
    logComposerPaste('paste event image count', images.length)
  }

  // WebKitGTK frequently leaves the synchronous paste event without image
  // data. Fall back to the async Clipboard API on Ctrl/Cmd+V.
  const handleComposerKeyDown = async (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (isSendKey(e, sendShortcut)) {
      e.preventDefault()
      void handleSendReply()
      return
    }
    const isPaste = (e.ctrlKey || e.metaKey) && (e.key === 'v' || e.key === 'V')
    if (isPaste) {
      logComposerPaste('paste shortcut keydown', {
        key: e.key,
        ctrlKey: e.ctrlKey,
        metaKey: e.metaKey,
        target: e.currentTarget.tagName,
      })
    }
    if (!isPaste) return

    const pasteStartedAt = Date.now()
    void readClipboardImages()
      .then((images) => {
        if (lastImagePasteAtRef.current >= pasteStartedAt) return
        logComposerPaste('async clipboard image count', images.length)
        if (images.length === 0) {
          return readNativeClipboardImage().then((image) => {
            if (!image || lastImagePasteAtRef.current >= pasteStartedAt) return
            lastImagePasteAtRef.current = Date.now()
            addNativeClipboardImage(image)
          })
        }
        lastImagePasteAtRef.current = Date.now()
        addAttachmentFiles(images)
      })
      .catch((error) => {
        logComposerPaste('async clipboard read failed', error)
        void readNativeClipboardImage()
          .then((image) => {
            if (!image || lastImagePasteAtRef.current >= pasteStartedAt) return
            lastImagePasteAtRef.current = Date.now()
            addNativeClipboardImage(image)
          })
          .catch((nativeError) => {
            logComposerPaste('native clipboard read failed', nativeError)
          })
      })
  }

  return {
    composer,
    composerAttachments,
    sendShortcut,
    sendingReply,
    textareaRef,
    handleSendReply,
    pickAttachmentFiles,
    handleComposerPaste,
    handleComposerKeyDown,
  }
}
