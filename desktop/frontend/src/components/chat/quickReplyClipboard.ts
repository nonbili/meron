import { invoke } from '../../lib/bridge'

const COMPOSER_PASTE_DEBUG = true

export type NativeClipboardImage = {
  filename: string
  mime: string
  size: number
  data: string
}

export function logComposerPaste(message: string, data?: unknown) {
  if (!COMPOSER_PASTE_DEBUG) return
  if (data === undefined) {
    console.debug(`[composer-paste] ${message}`)
  } else {
    console.debug(`[composer-paste] ${message}`, data)
  }
}

// Pull image files out of a synchronous paste/drop DataTransfer.
export function extractClipboardImages(data: DataTransfer | null): File[] {
  if (!data) {
    logComposerPaste('paste event has no clipboardData')
    return []
  }
  logComposerPaste('paste event clipboardData', {
    fileCount: data.files?.length ?? 0,
    itemCount: data.items?.length ?? 0,
    types: Array.from(data.types ?? []),
    items: data.items
      ? Array.from(data.items).map((item) => ({
          kind: item.kind,
          type: item.type,
        }))
      : [],
    files: data.files
      ? Array.from(data.files).map((file) => ({
          name: file.name,
          type: file.type,
          size: file.size,
        }))
      : [],
  })
  const images: File[] = []
  const seen = new Set<File>()

  const add = (file: File | null) => {
    if (!file || !file.type.toLowerCase().startsWith('image/') || seen.has(file)) return
    seen.add(file)
    images.push(file)
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

  return images
}

// Async Clipboard API read — WebKitGTK often omits images from the sync event.
export async function readClipboardImages(): Promise<File[]> {
  logComposerPaste('async clipboard availability', {
    hasClipboard: !!navigator.clipboard,
    hasRead: !!navigator.clipboard?.read,
  })
  if (!navigator.clipboard?.read) return []
  const items = await navigator.clipboard.read()
  logComposerPaste(
    'async clipboard items',
    items.map((item) => ({ types: item.types })),
  )
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

// Last-resort native bridge read for clipboard images.
export async function readNativeClipboardImage(): Promise<NativeClipboardImage | null> {
  const image = await invoke<NativeClipboardImage | null>('composer.readClipboardImage', {})
  logComposerPaste(
    'native clipboard image',
    image
      ? {
          filename: image.filename,
          mime: image.mime,
          size: image.size,
          dataLength: image.data.length,
        }
      : null,
  )
  return image
}
