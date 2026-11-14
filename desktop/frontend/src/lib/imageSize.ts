// Read an image's pixel dimensions from its header bytes, without decoding it.
// Covers the formats a composer attaches inline: PNG, JPEG, GIF and WebP.

export type ImageSize = { width: number; height: number }

// Enough of the file to reach a JPEG's SOF marker past typical EXIF blocks.
const HEADER_BYTES = 256 * 1024

function decodeHead(base64: string): Uint8Array | null {
  try {
    // Whole 4-char groups only, so the slice stays valid base64.
    const head = base64.slice(0, Math.ceil(HEADER_BYTES / 3) * 4)
    const binary = atob(head)
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
    return bytes
  } catch {
    return null
  }
}

const u16be = (b: Uint8Array, i: number) => (b[i] << 8) | b[i + 1]
const u16le = (b: Uint8Array, i: number) => b[i] | (b[i + 1] << 8)
const u24le = (b: Uint8Array, i: number) => b[i] | (b[i + 1] << 8) | (b[i + 2] << 16)
const u32be = (b: Uint8Array, i: number) => ((b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]) >>> 0
const ascii = (b: Uint8Array, i: number, n: number) => String.fromCharCode(...b.subarray(i, i + n))

function pngSize(b: Uint8Array): ImageSize | null {
  if (b.length < 24 || b[0] !== 0x89 || ascii(b, 1, 3) !== 'PNG' || ascii(b, 12, 4) !== 'IHDR') return null
  return { width: u32be(b, 16), height: u32be(b, 20) }
}

function gifSize(b: Uint8Array): ImageSize | null {
  if (b.length < 10 || ascii(b, 0, 3) !== 'GIF') return null
  return { width: u16le(b, 6), height: u16le(b, 8) }
}

function webpSize(b: Uint8Array): ImageSize | null {
  if (b.length < 30 || ascii(b, 0, 4) !== 'RIFF' || ascii(b, 8, 4) !== 'WEBP') return null
  const chunk = ascii(b, 12, 4)
  if (chunk === 'VP8X') return { width: u24le(b, 24) + 1, height: u24le(b, 27) + 1 }
  if (chunk === 'VP8 ') return { width: u16le(b, 26) & 0x3fff, height: u16le(b, 28) & 0x3fff }
  if (chunk === 'VP8L') {
    const bits = b[21] | (b[22] << 8) | (b[23] << 16) | (b[24] << 24)
    return { width: (bits & 0x3fff) + 1, height: ((bits >> 14) & 0x3fff) + 1 }
  }
  return null
}

// EXIF orientation (tag 0x0112) from an APP1 segment's payload, or 1 (upright)
// when absent. `start` is where the payload begins, `end` where the segment ends.
function exifOrientation(b: Uint8Array, start: number, end: number): number {
  if (end > b.length || ascii(b, start, 6) !== 'Exif\0\0') return 1
  const tiff = start + 6
  const order = ascii(b, tiff, 2)
  if (order !== 'II' && order !== 'MM') return 1
  const u16 = (i: number) => (order === 'II' ? u16le(b, i) : u16be(b, i))
  const u32 = (i: number) => (order === 'II' ? (u16le(b, i) | (u16le(b, i + 2) << 16)) >>> 0 : u32be(b, i))
  const ifd = tiff + u32(tiff + 4)
  if (ifd + 2 > end) return 1
  const entries = u16(ifd)
  for (let n = 0; n < entries; n++) {
    const entry = ifd + 2 + n * 12
    if (entry + 12 > end) break
    if (u16(entry) === 0x0112) return u16(entry + 8)
  }
  return 1
}

function jpegSize(b: Uint8Array): ImageSize | null {
  if (b.length < 4 || b[0] !== 0xff || b[1] !== 0xd8) return null
  let orientation = 1
  let i = 2
  while (i + 9 < b.length) {
    if (b[i] !== 0xff) return null
    const marker = b[i + 1]
    if (marker === 0xff) {
      i++
      continue
    }
    const length = u16be(b, i + 2)
    if (marker === 0xe1) orientation = exifOrientation(b, i + 4, i + 2 + length)
    // Start-of-frame markers, excluding DHT (C4), JPG (C8) and DAC (CC).
    if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
      const width = u16be(b, i + 7)
      const height = u16be(b, i + 5)
      // Orientations 5–8 turn the image a quarter, so it displays sideways
      // to how it is stored.
      return orientation >= 5 && orientation <= 8 ? { width: height, height: width } : { width, height }
    }
    i += 2 + length
  }
  return null
}

/** Pixel size of a base64-encoded image as displayed, or null when unknown.
 * A JPEG's EXIF orientation is applied, so a photo stored sideways reports its
 * upright size. */
export function imageSize(base64: string): ImageSize | null {
  const bytes = decodeHead(base64)
  if (!bytes) return null
  const size = pngSize(bytes) ?? gifSize(bytes) ?? webpSize(bytes) ?? jpegSize(bytes)
  if (!size || size.width <= 0 || size.height <= 0) return null
  return size
}

/** Scale `size` down (never up) to fit inside `max`, keeping its aspect ratio. */
export function fitImageSize(size: ImageSize, max: ImageSize): ImageSize {
  const scale = Math.min(1, max.width / size.width, max.height / size.height)
  return {
    width: Math.max(1, Math.round(size.width * scale)),
    height: Math.max(1, Math.round(size.height * scale)),
  }
}
