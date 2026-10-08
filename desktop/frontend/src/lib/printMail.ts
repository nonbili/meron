import { invoke } from './bridge'
import { CONVERSATION_PAGE_SIZE } from './pagination'
import { htmlToText } from './html'
import { t } from './i18n'
import { formatFullTimestamp, htmlReferencesMedia } from '../components/chat/messageHelpers'
import { showToast, ui$ } from '../states/ui'
import { accounts$ } from '../states/accounts'
import { settings$ } from '../states/settings'
import { thread$ } from '../states/thread'
import { remoteContentAllowed } from '../components/chat/messageHelpers'
import { stripTrackingPixels } from '../components/chat/readerHtml'
import { applyRemoteContentPolicy } from '../components/chat/remoteContentCsp'
import type { Message, MessageTab } from '../types'

let disposePrint: (() => void) | undefined

function printPreparationFeedback() {
  const message = t('chat.preparingPrint')
  let shown = false
  const timer = window.setTimeout(() => {
    shown = true
    showToast(message, 'success', 0)
  }, 1000)
  return () => {
    window.clearTimeout(timer)
    if (shown && ui$.toast.peek() === message) {
      ui$.toast.set('')
      ui$.toastUndo.set(null)
    }
  }
}

function printAttachmentNames(mail: Message | MessageTab, renderedHtml = false): string[] {
  const html = 'from_addr' in mail ? mail.body_html : mail.bodyHtml
  return (mail.attachments ?? [])
    .filter((attachment) => !renderedHtml || !htmlReferencesMedia(html, attachment))
    .map((attachment) => attachment.filename)
}

// Plain mode and messages without HTML retain a text-only print document.
export function mailPrintText(mail: Message | MessageTab): string {
  const message = 'from_addr' in mail
  const from = message
    ? mail.from_name
      ? `${mail.from_name} <${mail.from_addr}>`
      : mail.from_addr
    : mail.fromRaw || mail.from
  const rows = [
    mail.subject || t('threads.noSubject'),
    `${t('composer.fields.from')}: ${from}`,
    ...(['to', 'cc', 'bcc'] as const).flatMap((key) =>
      mail[key] ? [`${t(`composer.fields.${key}`)}: ${mail[key]}`] : [],
    ),
    (message ? mail.reply_to : mail.replyTo) ? `${t('chat.replyTo')}: ${message ? mail.reply_to : mail.replyTo}` : '',
    mail.date ? formatFullTimestamp(mail.date) : '',
  ].filter(Boolean)
  let body = (message ? mail.body_missing : mail.bodyMissing) ? t('chat.couldNotPrintMessage') : mail.body
  if (!body) {
    const html = new DOMParser().parseFromString((message ? mail.body_html : mail.bodyHtml) || '', 'text/html')
    html
      .querySelectorAll('script, style, img, iframe, object, embed, link, video, audio, source')
      .forEach((el) => el.remove())
    html.querySelectorAll('*').forEach((el) => {
      for (const attribute of Array.from(el.attributes)) el.removeAttribute(attribute.name)
    })
    body = htmlToText(html.body.innerHTML)
  }
  const attachments = printAttachmentNames(mail)
  return `${rows.join('\n')}\n\n${body}${attachments.length ? `\n\n${t('chat.attachments')}:\n${attachments.join('\n')}` : ''}`
}

export async function loadPrintThread(threadId: string): Promise<Message[]> {
  const messages = new Map<string, Message>()
  const cursors = new Set<string>()
  let cursor = ''
  do {
    if (cursors.has(cursor)) throw new Error('Repeated thread cursor')
    cursors.add(cursor)
    const page = await invoke<{ messages: Message[]; next_cursor?: string }>('mail.threadRead', {
      thread_id: threadId,
      limit: CONVERSATION_PAGE_SIZE,
      for_print: true,
      ...(cursor ? { before_cursor: cursor } : {}),
    })
    for (const message of page.messages) {
      if (!messages.has(message.id)) messages.set(message.id, message)
    }
    cursor = page.next_cursor ?? ''
  } while (cursor)
  if (!messages.size) throw new Error('Thread is empty')
  return [...messages.values()].sort((a, b) => a.date - b.date)
}

export async function printThread(threadId: string) {
  const stopPreparing = printPreparationFeedback()
  try {
    await printMails(await loadPrintThread(threadId), 'chat.couldNotPrintThread', undefined, stopPreparing)
  } catch {
    showToast(t('chat.couldNotPrintThread'), 'error')
  } finally {
    stopPreparing()
  }
}

export async function printMail(mail: Message | MessageTab, allowRemote?: boolean) {
  const stopPreparing = printPreparationFeedback()
  try {
    await printMails([mail], 'chat.couldNotPrintMessage', allowRemote, stopPreparing)
  } finally {
    stopPreparing()
  }
}

/** Keep the backend-sanitized email and its CSS/CSP in a separate document. */
export function mailPrintHtml(mail: Message | MessageTab, allowRemote: boolean): string | undefined {
  const message = 'from_addr' in mail
  if (message) {
    const account = accounts$.peek().find((account) => account.id === mail.account_id)
    const mode =
      thread$.conversationModeOverrides.peek()[mail.account_id] ??
      (account ? ((account.conversation_html ?? true) ? 'html' : 'plain') : 'plain')
    if (mode === 'plain') return
  }
  if (message ? mail.body_missing : mail.bodyMissing || mail.viewMode === 'plain') return
  const html = message ? mail.body_html : mail.bodyHtml
  if (!html) return
  const doc = new DOMParser().parseFromString(
    applyRemoteContentPolicy(stripTrackingPixels(html), allowRemote),
    'text/html',
  )
  // Always enforce a print policy, including for HTML without a baked CSP.
  const policy = doc.createElement('meta')
  policy.httpEquiv = 'Content-Security-Policy'
  policy.content = `default-src 'none'; script-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'; style-src 'unsafe-inline'; img-src 'self' data: ${allowRemote ? 'http: https:' : ''}; media-src 'self' data: blob: ${allowRemote ? 'http: https:' : ''}; font-src 'self' data:;`
  doc.head.prepend(policy)
  doc
    .querySelectorAll('script, iframe, object, embed, base, link, meta[http-equiv="refresh" i]')
    .forEach((el) => el.remove())
  doc.querySelectorAll('img').forEach((img) => {
    img.loading = 'eager'
  })
  // Keep headers and body in one pagination context. A separately measured
  // iframe can otherwise move the entire body to the page after the headers.
  const header = doc.createElement('pre')
  header.className = 'meron-print-summary'
  header.textContent = mailPrintText({ ...mail, body: ' ', attachments: [] }).trimEnd()
  header.style.marginBottom = '16px'
  doc.body.prepend(header)
  const attachmentNames = printAttachmentNames(mail, true)
  if (attachmentNames.length) {
    const attachments = doc.createElement('pre')
    attachments.className = 'meron-print-summary'
    attachments.textContent = `\n${t('chat.attachments')}:\n${attachmentNames.join('\n')}`
    doc.body.append(attachments)
  }
  const style = doc.createElement('style')
  style.textContent = `html, body { height: auto !important; overflow: visible !important; }
    body { margin: 0; overflow-wrap: anywhere; }
    img, video, table { max-width: 100% !important; }
    img { height: auto !important; }
    pre { white-space: pre-wrap; overflow-wrap: anywhere; }
    body > pre.meron-print-summary { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere;
      font: 11pt/1.5 sans-serif; color: black; }
    * { -webkit-print-color-adjust: exact; print-color-adjust: exact; }`
  doc.head.append(style)
  return '<!doctype html>' + doc.documentElement.outerHTML
}

function printRemoteAllowed(mail: Message | MessageTab): boolean {
  if (!('from_addr' in mail)) return !!mail.revealRemote
  return (
    remoteContentAllowed(
      mail,
      accounts$.peek().find((account) => account.id === mail.account_id),
      settings$.remoteImageSenders.peek(),
    ) || !!thread$.revealedRemote.peek()[mail.id]
  )
}

// Attributes that fetch something once printed; mailPrintHtml already drops
// scripts, frames, objects and stylesheet links. href counts outside links (SVG
// image/use); metadata such as GitHub's itemtype="http://schema.org/…" does not.
const LOADING_ATTRIBUTES = new Set(['src', 'srcset', 'poster', 'background', 'style', 'href', 'xlink:href'])
const NAVIGATING_ELEMENTS = new Set(['a', 'area', 'link'])

// A URL attribute is remote when it starts with a scheme or is protocol-relative;
// inline data (a data: GIF can hold "///") is not. CSS is checked anywhere a URL
// can start: after a quote, bracket or whitespace.
const REMOTE_URL = /^\s*(https?:|\/\/)/i
const REMOTE_CSS_PATTERN = /https?:|['"(\s]\/\//i

// CSS escapes can spell a URL (url(\68 ttps://…) is https://…), so they are
// decoded first: hex escapes with their optional trailing whitespace, any other
// escaped character as itself, and line continuations (backslash–newline, which
// a string drops) removed. Outside strings a continuation is invalid; dropping
// it there too only errs towards remote.
function decodeCssEscapes(css: string): string {
  return css.replace(/\\([0-9a-f]{1,6})(?:\r\n|[ \t\n\r\f])?|\\(\r\n|[\s\S])/gi, (_, hex, char) => {
    if (hex === undefined) return /^(\r\n|[\n\r\f])$/.test(char) ? '' : char
    const code = parseInt(hex, 16)
    return code === 0 || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff) ? '\ufffd' : String.fromCodePoint(code)
  })
}

const remoteCss = (css: string) => REMOTE_CSS_PATTERN.test(decodeCssEscapes(css))

/** Whether an email body could load anything remote once printed. Errs towards
 *  yes for anything that fetches; links and text are ignored. */
export function referencesRemoteContent(html: string | undefined): boolean {
  if (!html) return false
  const doc = new DOMParser().parseFromString(stripTrackingPixels(html), 'text/html')
  for (const el of doc.querySelectorAll('*')) {
    if (el.localName === 'style' && remoteCss(el.textContent ?? '')) return true
    for (const { name, value } of Array.from(el.attributes)) {
      if (!LOADING_ATTRIBUTES.has(name)) continue
      if (name === 'href' && NAVIGATING_ELEMENTS.has(el.localName)) continue
      if (
        name === 'style'
          ? remoteCss(value)
          : name === 'srcset'
            ? value.split(',').some((candidate) => REMOTE_URL.test(candidate))
            : REMOTE_URL.test(value)
      )
        return true
    }
  }
  return false
}

// A thread whose messages print under different remote policies cannot share
// one document, and falls back to per-message frames that slice lines at page
// edges. A message with nothing remote prints the same under the stricter one.
function printRemoteDecisions(mails: (Message | MessageTab)[], allowRemote?: boolean): boolean[] {
  const allowed = mails.map((mail) => allowRemote ?? printRemoteAllowed(mail))
  if (!allowed.includes(true) || !allowed.includes(false)) return allowed
  return allowed.map(
    (allow, index) =>
      allow && referencesRemoteContent('from_addr' in mails[index] ? mails[index].body_html : mails[index].bodyHtml),
  )
}

// Shadow roots isolate each email's CSS without introducing iframe page slicing.
// Native WebKit on Linux/macOS, or pagedPrint elsewhere, attaches these templates before printing.
export function nativePrintHtml(root: HTMLElement): string | undefined {
  const doc = document.implementation.createHTMLDocument('')
  const title = doc.createElement('title')
  title.textContent = root.dataset.printTitle || t('threads.noSubject')
  doc.head.querySelector('title')?.remove()
  doc.head.append(title)
  const policy = doc.createElement('meta')
  policy.httpEquiv = 'Content-Security-Policy'
  policy.content = "script-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'"
  doc.head.append(policy)
  const style = doc.createElement('style')
  style.textContent = `@page { margin: 0; }
    body { margin: 0; color: black; background: white; }
    section + section { break-before: page; page-break-before: always; }
    pre { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; font: 11pt/1.5 sans-serif; }`
  doc.head.append(style)
  let resourcePolicy: string | undefined
  for (const source of root.querySelectorAll(':scope > section')) {
    const section = doc.createElement('section')
    const frame = source.querySelector('iframe')
    if (frame) {
      if (!frame.contentDocument?.body) throw new Error('Print HTML unavailable')
      const mail = frame.contentDocument
      const policies = Array.from(mail.querySelectorAll('meta[http-equiv="Content-Security-Policy" i]'))
      const signature = policies
        .map((meta) => meta.getAttribute('content'))
        .sort()
        .join('\n')
      // A top-level document cannot enforce different CSPs per shadow root.
      // Keep the isolated-frame fallback for mixed-policy threads rather than
      // intersecting policies (missing images) or broadening them (remote leaks).
      if (resourcePolicy !== undefined && resourcePolicy !== signature) return undefined
      if (resourcePolicy === undefined) policies.forEach((meta) => doc.head.append(doc.importNode(meta, true)))
      resourcePolicy = signature
      const template = doc.createElement('template')
      template.setAttribute('data-print-message', '')
      const html = doc.importNode(mail.documentElement, true)
      const summaries = html.querySelectorAll('body > pre.meron-print-summary')
      summaries.forEach((summary) => summary.remove())
      if (summaries[0]) section.append(summaries[0])
      html.querySelectorAll('meta, script').forEach((el) => el.remove())
      // Store as escaped text so the outer HTML parser cannot discard the
      // nested html/head/body elements needed by the email's CSS selectors.
      template.content.append(doc.createTextNode(html.outerHTML))
      // Only the email body is a shadow host. Attaching to section would hide
      // its light-DOM header and attachment summary (there is no slot).
      const host = doc.createElement('div')
      host.setAttribute('data-print-body', '')
      host.append(template)
      section.append(host)
      for (const summary of Array.from(summaries).slice(1)) section.append(summary)
    } else {
      section.innerHTML = source.innerHTML
    }
    doc.body.append(section)
  }
  return '<!doctype html>' + doc.documentElement.outerHTML
}

async function printMails(
  mails: (Message | MessageTab)[],
  errorKey = 'chat.couldNotPrintMessage',
  allowRemote?: boolean,
  stopPreparing: () => void = () => {},
) {
  disposePrint?.()
  const root = document.createElement('div')
  root.id = 'meron-print-document'
  const printTitle = mails[0]?.subject || t('threads.noSubject')
  root.dataset.printTitle = printTitle
  const originalTitle = document.title
  let changedTitle = false
  const style = document.createElement('style')
  style.textContent = `
    #meron-print-document { position: absolute; left: -100000px; top: 0; width: 180mm; }
    @media print {
      @page { margin: 15mm; }
      html, body { height: auto !important; overflow: visible !important; background: white !important; }
      body > :not(#meron-print-document) { display: none !important; }
      #meron-print-document { position: static; width: auto; display: block !important; color: black; background: white; }
      #meron-print-document > section + section { break-before: page; page-break-before: always; }
      #meron-print-document pre { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere;
        font: 11pt/1.5 sans-serif; color: black; }
    }`
  root.append(style)
  const ready: Promise<void>[] = []
  const frames: HTMLIFrameElement[] = []
  const timers: number[] = []
  const measureFrames = (targets = frames) => {
    // Batch writes before reads to avoid a layout flush for every frame.
    for (const frame of targets) frame.style.height = '1px'
    const heights = targets.map((frame) => {
      const doc = frame.contentDocument
      return doc?.body ? Math.max(doc.body.scrollHeight, doc.documentElement.scrollHeight) : 1
    })
    targets.forEach((frame, index) => {
      frame.style.height = `${heights[index]}px`
    })
  }
  const beforePrint = () => measureFrames()
  // WebKitGTK prints iframes blank while it draws its own overlay scrollbars
  // (html.native-scrollbars, lib/nativeScrollbars); the styled ones are restored
  // to print. macOS has the class too and takes the same path, harmlessly.
  const nativeScrollbars = document.documentElement.classList.contains('native-scrollbars')
  const printPage = (win: Window) => {
    document.documentElement.classList.remove('native-scrollbars')
    win.print()
  }
  const browserPrint = () => {
    // Browser fallback retains iframe CSP isolation; summaries belong outside
    // that email-controlled style scope.
    for (const frame of frames) {
      const summaries = frame.contentDocument?.querySelectorAll('body > pre.meron-print-summary')
      if (summaries?.[0]) frame.before(summaries[0])
      for (const summary of Array.from(summaries ?? []).slice(1)) frame.after(summary)
    }
    measureFrames()
    stopPreparing()
    document.title = printTitle
    changedTitle = true
    printPage(window)
  }
  // Browsers slice iframes at page edges instead of breaking between lines,
  // so print the shadow-root document from its own frame as a top-level page.
  const pagedPrint = async (html: string) => {
    const frame = document.createElement('iframe')
    // No allow-scripts: the frame's CSP and sandbox both keep mail markup inert.
    frame.setAttribute('sandbox', 'allow-same-origin allow-modals')
    frame.style.cssText = 'display:block;width:100%;border:0;height:1px;'
    const loaded = new Promise<void>((resolve) => {
      frame.onload = () => resolve()
    })
    frame.srcdoc = html
    root.append(frame)
    await loaded
    const doc = frame.contentDocument
    const win = frame.contentWindow
    if (!doc?.body || !win) throw new Error('Print HTML unavailable')
    const margins = doc.createElement('style')
    margins.textContent = '@page { margin: 15mm; }'
    doc.head.append(margins)
    const images: HTMLImageElement[] = []
    for (const template of doc.querySelectorAll<HTMLTemplateElement>('template[data-print-message]')) {
      const mail = new DOMParser().parseFromString(template.content.textContent ?? '', 'text/html')
      const shadow = template.parentElement!.attachShadow({ mode: 'open' })
      shadow.append(doc.importNode(mail.documentElement, true))
      template.remove()
      images.push(...shadow.querySelectorAll('img'))
    }
    // Template images start loading only after attachment.
    await new Promise<void>((resolve) => {
      timers.push(window.setTimeout(resolve, 15000))
      Promise.all(
        images.map(
          (img) =>
            img.complete ||
            new Promise((done) => {
              img.addEventListener('load', done, { once: true })
              img.addEventListener('error', done, { once: true })
            }),
        ),
      ).then(() => resolve())
    })
    if (!root.isConnected) return
    stopPreparing()
    win.addEventListener('afterprint', cleanup, { once: true })
    printPage(win)
  }
  const remoteDecisions = printRemoteDecisions(mails, allowRemote)
  for (const [index, mail] of mails.entries()) {
    const section = document.createElement('section')
    root.append(section)
    const content = document.createElement('pre')
    const html = mailPrintHtml(mail, remoteDecisions[index])
    if (!html) {
      content.textContent = mailPrintText(mail)
      section.append(content)
      continue
    }
    const frame = document.createElement('iframe')
    frame.title = mail.subject
    frame.setAttribute('sandbox', 'allow-same-origin')
    frame.style.cssText = 'display:block;width:100%;border:0;height:1px;'
    frames.push(frame)
    ready.push(
      new Promise<void>((resolve, reject) => {
        const finish = () => {
          window.clearTimeout(timeout)
          const doc = frame.contentDocument
          if (!doc?.body) {
            reject(new Error('Print HTML unavailable'))
            return
          }
          measureFrames([frame])
          resolve()
        }
        // Images may never finish loading; print the available document instead.
        const timeout = window.setTimeout(finish, 15000)
        timers.push(timeout)
        frame.onload = finish
      }),
    )
    frame.srcdoc = html
    section.append(frame)
  }
  document.body.append(root)
  const cleanup = () => {
    stopPreparing()
    if (changedTitle && document.title === printTitle) document.title = originalTitle
    if (nativeScrollbars) document.documentElement.classList.add('native-scrollbars')
    root.remove()
    timers.forEach((timer) => window.clearTimeout(timer))
    window.removeEventListener('beforeprint', beforePrint)
    window.removeEventListener('afterprint', cleanup)
    if (disposePrint === cleanup) disposePrint = undefined
  }
  disposePrint = cleanup
  window.addEventListener('afterprint', cleanup, { once: true })
  window.addEventListener('beforeprint', beforePrint)
  try {
    await Promise.all(ready)
    if (!root.isConnected) return
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    const html = nativePrintHtml(root)
    if (html !== undefined && (window as any).go?.main?.App && (await invoke<boolean>('mail.print', { html })))
      cleanup()
    else if (html !== undefined) await pagedPrint(html)
    else browserPrint()
  } catch {
    cleanup()
    showToast(t(errorKey), 'error')
  }
}
