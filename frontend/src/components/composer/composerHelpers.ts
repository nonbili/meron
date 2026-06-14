import type { ComposerAttachment } from '../../types'

export function textToHtml(text: string): string {
  if (!text.trim()) return ''
  return text
    .split(/\n{2,}/)
    .map(
      (para) =>
        `<p>${para
          .split('\n')
          .map((line) => line.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'))
          .join('<br>')}</p>`,
    )
    .join('')
}

export const createInlineId = () => `meron-image-${Date.now()}-${Math.random().toString(36).substring(2, 9)}@meron`

// The composer renders rich formatting (blockquotes, code, links) purely through
// Meron's stylesheet. Receiving clients don't have that CSS, so we bake the
// equivalent styles into inline `style` attributes before sending/saving. Uses
// concrete colors since CSS custom properties don't resolve in email clients.
const RICH_INLINE_STYLES: Record<string, string> = {
  blockquote: 'border-left: 3px solid #d1d5db; padding-left: 0.9em; margin: 0.6em 0; color: #6b7280;',
  a: 'color: #2563eb; text-decoration: underline;',
  code: 'background: rgba(127, 127, 127, 0.16); border-radius: 4px; padding: 0.1em 0.3em; font-size: 0.9em;',
}

export function inlineRichStyles(html: string): string {
  if (!html) return html
  const parser = new DOMParser()
  const doc = parser.parseFromString(html, 'text/html')
  for (const [selector, style] of Object.entries(RICH_INLINE_STYLES)) {
    for (const el of Array.from(doc.querySelectorAll(selector))) {
      // Preserve any existing inline styles; only prepend our defaults.
      const existing = el.getAttribute('style')
      el.setAttribute('style', existing ? `${style} ${existing}` : style)
    }
  }
  return doc.body.innerHTML
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onloadend = () => {
      const dataUrl = reader.result as string
      const base64Data = dataUrl.split(',')[1]
      resolve(base64Data)
    }
    reader.onerror = reject
    reader.readAsDataURL(blob)
  })
}

// Rewrite the body's <img> tags into cid: references and collect the matching
// inline attachments. Data-URL and fetchable images become new attachments;
// already-cid images are kept; remote http(s) images are left untouched. Returns
// the cid-ified html plus attachments pruned to those still referenced.
export async function prepareInlineImages(html: string, initialAttachments: ComposerAttachment[]) {
  const parser = new DOMParser()
  const doc = parser.parseFromString(html, 'text/html')
  const imgs = Array.from(doc.querySelectorAll('img'))
  if (imgs.length === 0) return { html, attachments: initialAttachments }

  const attachments = [...initialAttachments]
  const usedInlineIds = new Set<string>()

  for (const img of imgs) {
    const src = img.getAttribute('src')
    if (!src) continue

    if (src.startsWith('cid:')) {
      const cid = src.slice(4)
      usedInlineIds.add(cid)
      continue
    }

    if (src.startsWith('http://') || src.startsWith('https://')) {
      continue
    }

    let mime = ''
    let base64Data = ''
    let size = 0

    if (src.startsWith('data:')) {
      const parts = src.split(',')
      if (parts.length >= 2) {
        const meta = parts[0]
        base64Data = parts[1]
        mime = meta.split(';')[0].slice(5)
        size = Math.floor((base64Data.length * 3) / 4)
      }
    } else if (
      !src.startsWith('http://') &&
      !src.startsWith('https://') &&
      !src.startsWith('cid:') &&
      !src.startsWith('data:')
    ) {
      try {
        const response = await fetch(src)
        const blob = await response.blob()
        mime = blob.type
        size = blob.size
        base64Data = await blobToBase64(blob)
      } catch (e) {
        console.error('Failed to fetch inline image from URL:', src, e)
        continue
      }
    }

    if (base64Data) {
      let existing = attachments.find((a) => a.data === base64Data)
      if (existing) {
        if (!existing.inlineId) {
          existing.inlineId = createInlineId()
        }
        img.setAttribute('src', `cid:${existing.inlineId}`)
        usedInlineIds.add(existing.inlineId)
      } else {
        const ext = mime.startsWith('image/') ? mime.slice(6) : 'png'
        const filename = `inline-image-${Date.now()}-${Math.random().toString(36).substring(2, 5)}.${ext}`
        const inlineId = createInlineId()
        const newAtt: ComposerAttachment = {
          id: `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`,
          filename,
          mime: mime || 'image/png',
          size: size || base64Data.length,
          data: base64Data,
          inlineId,
        }
        attachments.push(newAtt)
        img.setAttribute('src', `cid:${inlineId}`)
        usedInlineIds.add(inlineId)
      }
    }
  }

  return {
    html: doc.body.innerHTML,
    attachments: attachments.filter((att) => !att.inlineId || usedInlineIds.has(att.inlineId)),
  }
}

export function extractClipboardImages(data: DataTransfer | null): File[] {
  if (!data) return []
  const out: File[] = []
  const seen = new Set<File>()

  const add = (file: File | null) => {
    if (!file || !file.type.toLowerCase().startsWith('image/') || seen.has(file)) return
    seen.add(file)
    out.push(file)
  }

  if (data.files && data.files.length > 0) {
    for (let i = 0; i < data.files.length; i++) add(data.files[i])
  }
  if (data.items) {
    for (let i = 0; i < data.items.length; i++) {
      const item = data.items[i]
      if (item.kind === 'file' && item.type.toLowerCase().startsWith('image/')) {
        add(item.getAsFile())
      }
    }
  }
  return out
}

export async function readClipboardImages(): Promise<File[]> {
  if (!navigator.clipboard?.read) return []
  const items = await navigator.clipboard.read()
  const images: File[] = []
  for (const item of items) {
    const imgType = item.types.find((type) => type.toLowerCase().startsWith('image/'))
    if (!imgType) continue
    const blob = await item.getType(imgType)
    const ext = imgType.split('/')[1] || 'png'
    images.push(new File([blob], `pasted-image-${Date.now()}.${ext}`, { type: imgType }))
  }
  return images
}
