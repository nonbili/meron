import { useEffect, useRef } from 'react'
import type { RefObject } from 'react'

// Distance from the bottom at which a paged list asks for its next page — the
// same threshold its scroll handler uses.
export const LOAD_MORE_THRESHOLD = 240

// Paged lists load their next page on scroll, but a page narrowed by a filter
// can leave too few rows to scroll at all, and then no scroll ever comes. Keep
// asking for pages while the list doesn't reach past the threshold. A page
// that shows nothing new stops it — a filter matching nothing would otherwise
// read the whole folder — and the Load more button is left for the rest.
export function useFillScroller(
  ref: RefObject<HTMLElement | null>,
  {
    viewKey,
    count,
    canLoad,
    busy,
    load,
  }: { viewKey: string; count: number; canLoad: boolean; busy: boolean; load: () => void },
) {
  const last = useRef<{ viewKey: string; count: number } | null>(null)
  const loadRef = useRef(load)
  loadRef.current = load

  useEffect(() => {
    if (busy) return
    const el = ref.current
    // Only after a page that grew the list (or on the first one): an empty
    // list keeps its explicit button.
    if (!el || !canLoad || count === 0) return
    if (last.current?.viewKey === viewKey && last.current.count === count) return
    if (el.scrollHeight - el.scrollTop - el.clientHeight >= LOAD_MORE_THRESHOLD) return
    last.current = { viewKey, count }
    loadRef.current()
  }, [ref, viewKey, count, canLoad, busy])
}
