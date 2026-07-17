import { useLayoutEffect, useRef, useState } from 'react'
import { ExternalLink, Play } from 'lucide-react'
import { clearMediaSession } from '../../lib/mediaSession'
import { openExternal } from '../../lib/native'

interface VideoAttachmentProps {
  src: string
  externalUrl: string
  externalLabel: string
  className?: string
  videoClassName?: string
  posterClassName?: string
}

// Click-to-load video. A live <video> immediately spins up a media pipeline and
// registers an OS media session (an MPRIS player on Linux/WebKitGTK), so a thread
// with several videos would flood the system with "now playing" notifications.
// We show a lightweight poster until the user actually starts a clip, mounting
// the real <video> only then so just the played clip claims a session.
// The corner button opens the source in the system player as a fallback when
// the in-app webview can't decode the codec (common on Linux/WebKitGTK).
export function VideoAttachment({
  src,
  externalUrl,
  externalLabel,
  className = 'group relative w-full max-w-[320px]',
  videoClassName = 'w-full max-h-80 rounded-lg border border-border/20 bg-black',
  posterClassName = 'flex aspect-video w-full items-center justify-center rounded-lg border border-border/20 bg-black text-white/90 transition-colors hover:text-white cursor-pointer',
}: VideoAttachmentProps) {
  const [active, setActive] = useState(false)
  const videoRef = useRef<HTMLVideoElement | null>(null)

  // WebKitGTK doesn't drop the media pipeline (and its lingering MPRIS "now
  // playing" entry) just because the <video> unmounts when we switch threads.
  // Pausing, detaching the source, and calling load() forces the session to be
  // released on teardown.
  useLayoutEffect(() => {
    const el = videoRef.current
    return () => {
      if (!el) return
      el.pause()
      el.removeAttribute('src')
      el.load()
      clearMediaSession()
    }
  }, [active])

  return (
    <div className={className}>
      {active ? (
        <video
          ref={videoRef}
          src={src}
          controls
          autoPlay
          preload="metadata"
          onPause={clearMediaSession}
          onEnded={clearMediaSession}
          className={videoClassName}
        />
      ) : (
        <button type="button" onClick={() => setActive(true)} className={posterClassName}>
          <span className="flex h-12 w-12 items-center justify-center rounded-full bg-black/50">
            <Play size={22} fill="currentColor" />
          </span>
        </button>
      )}
      {externalUrl && (
        <button
          type="button"
          onClick={() => openExternal(externalUrl)}
          title={externalLabel}
          className="absolute top-1.5 right-1.5 z-10 flex h-7 w-7 items-center justify-center rounded-full bg-black/45 text-white/90 opacity-0 group-hover:opacity-100 hover:bg-black/70 transition-opacity cursor-pointer"
        >
          <ExternalLink size={13} />
        </button>
      )}
    </div>
  )
}
