import { useState, useEffect, type ReactNode } from 'react'
import { useValue } from '@legendapp/state/react'
import { invoke } from '../../lib/bridge'
import { settings$ } from '../../states/settings'

const COLORS = [
  'from-indigo-400 to-indigo-500 shadow-indigo-500/10',
  'from-violet-400 to-violet-500 shadow-violet-500/10',
  'from-teal-400 to-teal-500 shadow-teal-500/10',
  'from-emerald-400 to-emerald-500 shadow-emerald-500/10',
  'from-rose-400 to-rose-500 shadow-rose-500/10',
  'from-amber-400 to-amber-500 shadow-amber-500/10',
  'from-sky-400 to-sky-500 shadow-sky-500/10',
  'from-fuchsia-400 to-fuchsia-500 shadow-fuchsia-500/10',
]

export function avatarColor(name: string) {
  let hash = 0
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash)
  }
  return COLORS[Math.abs(hash) % COLORS.length]
}

export function initials(value: string) {
  return value
    .split(/\s|@/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join('')
}

type SourceKind = 'manual' | 'gravatar' | 'favicon' | 'none'

type ResolvedImage = { src: string; kind: SourceKind }
const avatarCache = new Map<string, { image: ResolvedImage; expires: number }>()
const avatarKey = (email: string, size: number) => `${email.trim().toLowerCase()}@${size}`

function cachedAvatar(key: string): ResolvedImage | undefined {
  const entry = avatarCache.get(key)
  if (entry && entry.expires > Date.now()) return entry.image
  avatarCache.delete(key)
}

function rememberAvatar(key: string, image: ResolvedImage) {
  // Never persist a transient failure as initials. The core caches confirmed
  // misses; this small UI cache only keeps successfully resolved images.
  if (!image.src) return
  avatarCache.delete(key)
  avatarCache.set(key, { image, expires: Date.now() + 60 * 60 * 1000 })
  while (
    avatarCache.size > 512 ||
    [...avatarCache.values()].reduce((bytes, entry) => bytes + entry.image.src.length, 0) > 16 * 1024 * 1024
  ) {
    avatarCache.delete(avatarCache.keys().next().value!)
  }
}

// Run low-priority work (avatar resolution) once the browser is idle, so it
// never competes with the synchronous render/commit of a thread switch. Falls
// back to a macrotask where requestIdleCallback isn't available. Returns a
// canceller.
function onIdle(fn: () => void): () => void {
  const ric = window as unknown as {
    requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number
    cancelIdleCallback?: (handle: number) => void
  }
  if (typeof ric.requestIdleCallback === 'function') {
    const handle = ric.requestIdleCallback(fn, { timeout: 1000 })
    return () => ric.cancelIdleCallback?.(handle)
  }
  const handle = window.setTimeout(fn, 0)
  return () => window.clearTimeout(handle)
}

interface AvatarProps {
  /** Used to derive both the initials and the gradient color. */
  name: string
  /** Optional email address to automatically resolve real avatars. */
  email?: string
  /** Optional manual image URL; bypasses automatic email resolution. */
  src?: string
  /** Diameter in pixels. */
  size?: number
  className?: string
  /** Rendered in place of the letter initials when no image resolves (e.g. an
   *  RSS glyph for feed accounts). The gradient background and ring are kept. */
  fallback?: ReactNode
}

export function Avatar({ name, email, src, size = 40, className = '', fallback }: AvatarProps) {
  const showRealAvatars = useValue(settings$.showRealAvatars)
  const initialImage = showRealAvatars && email && !src ? cachedAvatar(avatarKey(email, size)) : undefined
  const [resolvedSrc, setResolvedSrc] = useState<string | undefined>(src || initialImage?.src)
  const [sourceKind, setSourceKind] = useState<SourceKind>(src ? 'manual' : (initialImage?.kind ?? 'none'))

  useEffect(() => {
    if (src) {
      setResolvedSrc(src)
      setSourceKind('manual')
      return
    }

    if (!showRealAvatars || !email) {
      setResolvedSrc(undefined)
      setSourceKind('none')
      return
    }

    const key = avatarKey(email, size)
    const cached = cachedAvatar(key)
    if (cached) {
      setResolvedSrc(cached.src)
      setSourceKind(cached.kind)
      return
    }

    let cancelled = false
    setResolvedSrc(undefined)
    setSourceKind('none')
    const cancelIdle = onIdle(() => {
      invoke<ResolvedImage>('avatar.resolve', { email, size: size * 2 })
        .then((image) => {
          rememberAvatar(key, image)
          if (cancelled) return
          setResolvedSrc(image.src || undefined)
          setSourceKind(image.src ? image.kind : 'none')
        })
        .catch(() => {
          // The initials remain visible if the core is unavailable.
        })
    })
    return () => {
      cancelled = true
      cancelIdle()
    }
  }, [src, email, showRealAvatars, size])

  const handleImageError = () => {
    if (email && !src) avatarCache.delete(avatarKey(email, size))
    setResolvedSrc(undefined)
    setSourceKind('none')
  }

  if (resolvedSrc && sourceKind === 'favicon') {
    return (
      <span
        style={{ width: size, height: size }}
        className={`flex shrink-0 items-center justify-center rounded-full bg-white ring-1 ring-black/5 dark:ring-white/10 ${className}`}
      >
        <img
          src={resolvedSrc}
          alt={name}
          onError={handleImageError}
          referrerPolicy="no-referrer"
          loading="lazy"
          decoding="async"
          style={{ width: Math.round(size * 0.58), height: Math.round(size * 0.58) }}
          className="object-contain"
        />
      </span>
    )
  }

  if (resolvedSrc && sourceKind !== 'none') {
    return (
      <img
        src={resolvedSrc}
        alt={name}
        onError={handleImageError}
        referrerPolicy="no-referrer"
        loading="lazy"
        decoding="async"
        style={{ width: size, height: size }}
        className={`shrink-0 rounded-full object-cover bg-hover ring-1 ring-black/5 dark:ring-white/10 ${className}`}
      />
    )
  }

  return (
    <div
      style={{ width: size, height: size, fontSize: Math.round(size * 0.34) }}
      className={`flex shrink-0 items-center justify-center rounded-full bg-gradient-to-tr font-semibold text-white shadow-sm ring-1 ring-white/10 saturate-[0.8] ${avatarColor(name)} ${className}`}
    >
      {fallback ?? initials(name)}
    </div>
  )
}
