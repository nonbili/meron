export function LinkHoverPreview({ url }: { url: string | null }) {
  if (!url) return null

  return (
    <div
      className="pointer-events-none absolute bottom-0 left-0 z-30 rounded-tr-md border border-border/70 bg-header/95 px-2.5 py-1.5 text-[11px] font-medium leading-snug text-secondary shadow-lg backdrop-blur"
      style={{ maxWidth: 'min(720px, 100%)' }}
    >
      <span className="block truncate">{url}</span>
    </div>
  )
}
