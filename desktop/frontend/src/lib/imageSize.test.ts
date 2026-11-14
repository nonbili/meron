import { describe, expect, it } from 'bun:test'
import { fitImageSize, imageSize } from './imageSize'

// 37x21 solid images, one per format (generated with ImageMagick).
const FIXTURES: Record<string, string> = {
  png: 'iVBORw0KGgoAAAANSUhEUgAAACUAAAAVAQMAAAD7KrXtAAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAADUExURf8AABniCTcAAAAHdElNRQfqCw4MOS9AS53hAAAAJXRFWHRkYXRlOmNyZWF0ZQAyMDI2LTExLTE0VDEyOjU3OjQ3KzAwOjAwsErT2gAAACV0RVh0ZGF0ZTptb2RpZnkAMjAyNi0xMS0xNFQxMjo1Nzo0NyswMDowMMEXa2YAAAAodEVYdGRhdGU6dGltZXN0YW1wADIwMjYtMTEtMTRUMTI6NTc6NDcrMDA6MDCWAkq5AAAADElEQVQI12NgGEgAAAB+AAH9tTdSAAAAAElFTkSuQmCC',
  gif: 'R0lGODlhJQAVAPAAAP8AAAAAACH5BAAAAAAALAAAAAAlABUAAAIahI+py+0Po5y02ouz3rz7D4biSJbmiaYqWgAAOw==',
  'webp lossy':
    'UklGRk4AAABXRUJQVlA4IEIAAABQAwCdASolABUAPpFGnkslo6KhpWgAsBIJZwDO3oAAK/fDcAD+7qY//2LOWwLx//7nA/7nA/7nA/jbB+29aoAAAAA=',
  'webp lossless': 'UklGRhwAAABXRUJQVlA4TA8AAAAvJAAFAAcQ/Y/+ByKi/wEA',
  'webp extended':
    'UklGRnIAAABXRUJQVlA4WAoAAAAQAAAAJAAAFAAAQUxQSAoAAAABB1DAiAhERP8DVlA4IEIAAABQAwCdASolABUAPpFGnkslo6KhpWgAsBIJZwDO3oAAK/fDcAD+7qY//2LOWwLx//7nA/7nA/7nA/jbB+29aoAAAAA=',
  jpeg: '/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAMCAgICAgMCAgIDAwMDBAYEBAQEBAgGBgUGCQgKCgkICQkKDA8MCgsOCwkJDRENDg8QEBEQCgwSExIQEw8QEBD/2wBDAQMDAwQDBAgEBAgQCwkLEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBD/wAARCAAVACUDAREAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAj/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/8QAFgEBAQEAAAAAAAAAAAAAAAAAAAcJ/8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAwDAQACEQMRAD8AnRDGqYAAAAAAAAAAAAAD/9k=',
}

describe('imageSize', () => {
  for (const [format, data] of Object.entries(FIXTURES)) {
    it(`reads a ${format} header`, () => {
      expect(imageSize(data)).toEqual({ width: 37, height: 21 })
    })
  }

  it('returns null for unknown or malformed data', () => {
    expect(imageSize(btoa('not an image at all, just text'))).toBeNull()
    expect(imageSize('%%%')).toBeNull()
    expect(imageSize('')).toBeNull()
  })
})

// A JPEG header with an EXIF orientation tag ahead of a 4032x3024 SOF0
// frame, the way phones store portrait photos.
function exifJpeg(orientation: number, order: 'II' | 'MM'): string {
  const u16 = (n: number) => (order === 'II' ? [n & 0xff, n >> 8] : [n >> 8, n & 0xff])
  const u32 = (n: number) =>
    order === 'II' ? [...u16(n & 0xffff), ...u16(n >>> 16)] : [...u16(n >>> 16), ...u16(n & 0xffff)]
  const tiff = [
    ...[...order].map((c) => c.charCodeAt(0)),
    ...u16(42),
    ...u32(8), // IFD0 right after the header
    ...u16(1), // one entry: Orientation, SHORT, count 1
    ...u16(0x0112),
    ...u16(3),
    ...u32(1),
    ...u16(orientation),
    0,
    0,
    ...u32(0), // no next IFD
  ]
  const exif = [...'Exif\0\0'].map((c) => c.charCodeAt(0)).concat(tiff)
  const app1 = [0xff, 0xe1, (exif.length + 2) >> 8, (exif.length + 2) & 0xff, ...exif]
  const sof0 = [0xff, 0xc0, 0, 17, 8, 3024 >> 8, 3024 & 0xff, 4032 >> 8, 4032 & 0xff, 3, ...Array(9).fill(0)]
  return btoa(String.fromCharCode(0xff, 0xd8, ...app1, ...sof0))
}

describe('imageSize with EXIF orientation', () => {
  for (const order of ['II', 'MM'] as const) {
    it(`swaps a quarter-turned photo to its upright size (${order})`, () => {
      expect(imageSize(exifJpeg(6, order))).toEqual({ width: 3024, height: 4032 })
      expect(imageSize(exifJpeg(8, order))).toEqual({ width: 3024, height: 4032 })
    })

    it(`keeps the stored size for upright and half-turned photos (${order})`, () => {
      expect(imageSize(exifJpeg(1, order))).toEqual({ width: 4032, height: 3024 })
      expect(imageSize(exifJpeg(3, order))).toEqual({ width: 4032, height: 3024 })
    })
  }

  it('fits a portrait phone photo inside the box, not just to its width', () => {
    const size = imageSize(exifJpeg(6, 'II'))!
    expect(fitImageSize(size, { width: 600, height: 480 })).toEqual({ width: 360, height: 480 })
  })
})

describe('fitImageSize', () => {
  const box = { width: 600, height: 480 }

  it('scales a tall screenshot to the height limit', () => {
    expect(fitImageSize({ width: 898, height: 2014 }, box)).toEqual({ width: 214, height: 480 })
  })

  it('scales a wide photo to the width limit', () => {
    expect(fitImageSize({ width: 4000, height: 3000 }, box)).toEqual({ width: 600, height: 450 })
  })

  it('never enlarges a small image', () => {
    expect(fitImageSize({ width: 120, height: 80 }, box)).toEqual({ width: 120, height: 80 })
  })
})
