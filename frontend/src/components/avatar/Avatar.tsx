import { useState, useEffect, type ReactNode } from 'react'
import { useValue } from '@legendapp/state/react'
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

function faviconForEmail(email: string) {
  const domain = email.trim().toLowerCase().split('@')[1]
  if (!domain) return undefined
  return `https://icons.duckduckgo.com/ip3/${domain}.ico`
}

// Native SHA-256 Hashing helper
type SourceKind = 'manual' | 'gravatar' | 'favicon' | 'none'

// Module-level cache of resolved avatar outcomes, keyed by `email@size`. Without
// it, every Avatar mount re-ran the SHA-256 + Gravatar fetch (and the 404 →
// favicon fallback), so switching threads with the participants panel open fired
// a burst of external requests through WebKitGTK's small connection pool. The
// cache makes re-mounts for an already-seen address synchronous.
const avatarCache = new Map<string, { src: string | undefined; kind: SourceKind }>()

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

async function computeSha256(message: string): Promise<string> {
  const msgBuffer = new TextEncoder().encode(message)
  const hashBuffer = await crypto.subtle.digest('SHA-256', msgBuffer)
  const hashArray = Array.from(new Uint8Array(hashBuffer))
  return hashArray.map((b) => b.toString(16).padStart(2, '0')).join('')
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
  const [resolvedSrc, setResolvedSrc] = useState<string | undefined>(src)
  const [sourceKind, setSourceKind] = useState<SourceKind>(src ? 'manual' : 'none')

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

    const cleanEmail = email.trim().toLowerCase()
    const cacheKey = `${cleanEmail}@${size}`

    // Reuse a previously resolved outcome (gravatar URL, favicon, or "no avatar")
    // synchronously — no SHA recompute, no fetch.
    const cached = avatarCache.get(cacheKey)
    if (cached) {
      setResolvedSrc(cached.src)
      setSourceKind(cached.kind)
      return
    }

    // Defer the SHA-256 + Gravatar resolution to idle time so a thread switch
    // that mounts many avatars at once doesn't get held up by the burst — the
    // initials placeholder is already on screen until this resolves.
    let cancelled = false
    const cancelIdle = onIdle(() => {
      computeSha256(cleanEmail)
        .then((hash) => {
          if (cancelled) return
          const url = `https://www.gravatar.com/avatar/${hash}?s=${size * 2}&d=404`
          avatarCache.set(cacheKey, { src: url, kind: 'gravatar' })
          setResolvedSrc(url)
          setSourceKind('gravatar')
        })
        .catch(() => {
          if (cancelled) return
          const favicon = faviconForEmail(cleanEmail)
          const kind: SourceKind = favicon ? 'favicon' : 'none'
          avatarCache.set(cacheKey, { src: favicon, kind })
          setResolvedSrc(favicon)
          setSourceKind(kind)
        })
    })
    return () => {
      cancelled = true
      cancelIdle()
    }
  }, [src, email, showRealAvatars, size])

  const handleImageError = () => {
    const cacheKey = email ? `${email.trim().toLowerCase()}@${size}` : ''
    if (sourceKind === 'gravatar' && email) {
      const favicon = faviconForEmail(email)
      const kind: SourceKind = favicon ? 'favicon' : 'none'
      // Remember that this address has no Gravatar so future mounts skip the
      // 404 round-trip and go straight to the favicon.
      if (cacheKey) avatarCache.set(cacheKey, { src: favicon, kind })
      setResolvedSrc(favicon)
      setSourceKind(kind)
      return
    }
    if (cacheKey) avatarCache.set(cacheKey, { src: undefined, kind: 'none' })
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
