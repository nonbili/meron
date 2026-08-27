// In-thread search highlighting for HTML message frames. The plain-text
// renderer wraps matches in <mark> while building its React tree; an HTML body
// is a live document we don't own, so matches are wrapped in the DOM instead —
// that way a search never changes how the message itself renders.

const HIT_CLASS = 'meron-search-hit'
const ACTIVE_CLASS = 'meron-search-hit-active'
const SKIP_TAGS = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEXTAREA'])

/** A slice of one text to wrap: [from, to) of that text, and which hit it is
 *  part of, counting from 0 in document order. */
export type MatchRange = [number, number, number]

/** Undo a previous highlight pass, restitching the text nodes it split. */
export function clearFrameHighlights(doc: Document) {
  const marks = doc.querySelectorAll<HTMLElement>(`mark.${HIT_CLASS}`)
  if (marks.length === 0) return
  for (const mark of marks) {
    const parent = mark.parentNode
    if (!parent) continue
    parent.replaceChild(doc.createTextNode(mark.textContent ?? ''), mark)
    parent.normalize()
  }
}

/** Every rendered text node in document order, skipping non-content elements. */
export function collectTextNodes(doc: Document): Text[] {
  const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement
      if (!parent || SKIP_TAGS.has(parent.tagName)) return NodeFilter.FILTER_REJECT
      return node.nodeValue ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT
    },
  })
  const nodes: Text[] = []
  for (let node = walker.nextNode(); node; node = walker.nextNode()) nodes.push(node as Text)
  return nodes
}

/**
 * Case-fold one character. Folding per character rather than per string keeps
 * each folded run traceable to its source, at the cost of the one lowercase
 * rule that reads its neighbours: a Greek capital sigma lowercases to final
 * "ς" at the end of a word and to "σ" elsewhere. Folding both forms to "σ"
 * settles that without needing the context — and, because the needle goes
 * through the same function, it settles it the same way on both sides.
 */
function foldChar(char: string): string {
  const lower = char.toLowerCase()
  return lower.includes('ς') ? lower.replaceAll('ς', 'σ') : lower
}

/** The folded form of a search query — no offsets to keep, just the folding. */
function foldQuery(query: string): string {
  let out = ''
  for (const char of query) out += foldChar(char)
  return out
}

/**
 * The folded haystack, plus the map back to source offsets. Folding is not
 * length-preserving — U+0130 "İ" becomes two code units — so folding in place
 * would slide every offset after such a character off the source text.
 * `sourceStart[i]` / `sourceEnd[i]` give the bounds of the source character
 * that produced folded position `i`, so a match maps back exactly whether or
 * not the fold expanded, and "İstanbul" still answers to a search for "i".
 */
function foldWithSourceMap(texts: string[]): { folded: string; sourceStart: number[]; sourceEnd: number[] } {
  let folded = ''
  const sourceStart: number[] = []
  const sourceEnd: number[] = []
  let source = 0

  for (const text of texts) {
    for (const char of text) {
      const lower = foldChar(char)
      folded += lower
      for (let unit = 0; unit < lower.length; unit += 1) {
        sourceStart.push(source)
        sourceEnd.push(source + char.length)
      }
      source += char.length
    }
  }

  return { folded, sourceStart, sourceEnd }
}

/**
 * Locate `needle` in the concatenation of `texts`, case-insensitively, and cut
 * each match back into per-text ranges: `ranges` maps a text's index to the
 * [from, to, hit) slices of it to wrap, in order, where `hit` is the match's
 * position in the document — a match spanning markup yields one slice per text
 * it covers, all carrying the same hit number, so the search bar can count and
 * navigate occurrences rather than messages. Split out from the DOM work so the
 * offset arithmetic — which is what makes a match span markup — is testable.
 */
export function matchRanges(texts: string[], rawNeedle: string): { hits: number; ranges: Map<number, MatchRange[]> } {
  const ranges = new Map<number, MatchRange[]>()
  const needle = foldQuery(rawNeedle)
  if (!needle) return { hits: 0, ranges }

  const { folded, sourceStart, sourceEnd } = foldWithSourceMap(texts)

  // Offset of each text within the source, so a match maps back to the text (or
  // texts) it falls in.
  const starts: number[] = []
  let total = 0
  for (const text of texts) {
    starts.push(total)
    total += text.length
  }

  let hits = 0
  let cursor = 0
  for (let at = folded.indexOf(needle); at !== -1; at = folded.indexOf(needle, at + needle.length)) {
    hits += 1
    // Source bounds of the match: the start of the character the match opens
    // on, through the end of the one it closes on — a whole character is
    // highlighted even when the query only covers part of its folded form.
    const matchStart = sourceStart[at]
    const matchEnd = sourceEnd[at + needle.length - 1]
    // Matches are found left to right and never overlap, so the scan resumes at
    // the last text the previous match touched — which may hold this one too.
    for (let index = cursor; index < texts.length; index += 1) {
      const start = starts[index]
      const end = start + texts[index].length
      if (end <= matchStart) continue
      if (start >= matchEnd) break
      const from = Math.max(matchStart, start) - start
      const to = Math.min(matchEnd, end) - start
      if (to > from) {
        const list = ranges.get(index)
        if (list) list.push([from, to, hits - 1])
        else ranges.set(index, [[from, to, hits - 1]])
      }
      cursor = index
    }
  }

  return { hits, ranges }
}

/**
 * Wrap every case-insensitive occurrence of `query` in a <mark>. `activeHit` is
 * the index of the occurrence the search is currently parked on within this
 * document (-1 when the search is parked elsewhere); it gets the stronger
 * highlight, matching the plain renderer.
 *
 * Matching runs over the concatenation of the body's text nodes rather than
 * each node on its own, so a query spanning markup ("hello world" across
 * `<span>Hello</span> <strong>world</strong>") still highlights — it is cut back
 * into one <mark> per node the match covers. Returns the number of hits and the
 * first <mark> of the active one, which the caller scrolls to.
 */
export function applyFrameHighlights(
  doc: Document,
  query: string,
  activeHit: number,
): { hits: number; activeMark: HTMLElement | null } {
  clearFrameHighlights(doc)
  // matchRanges folds the case of both sides itself, so the query goes in raw.
  const needle = query.trim()
  if (!needle || !doc.body) return { hits: 0, activeMark: null }

  const nodes = collectTextNodes(doc)
  if (nodes.length === 0) return { hits: 0, activeMark: null }

  const { hits, ranges } = matchRanges(
    nodes.map((node) => node.nodeValue ?? ''),
    needle,
  )

  let activeMark: HTMLElement | null = null
  for (const [index, list] of ranges) {
    const node = nodes[index]
    const text = node.nodeValue ?? ''
    const fragment = doc.createDocumentFragment()
    let cursor = 0
    for (const [from, to, hit] of list) {
      // Ranges arrive in order; a rewind would mean two matches landed on the
      // same character, which must not duplicate the text around it.
      if (to <= cursor) continue
      const start = Math.max(from, cursor)
      if (start > cursor) fragment.appendChild(doc.createTextNode(text.slice(cursor, start)))
      const mark = doc.createElement('mark')
      const active = hit === activeHit
      mark.className = active ? `${HIT_CLASS} ${ACTIVE_CLASS}` : HIT_CLASS
      mark.textContent = text.slice(start, to)
      fragment.appendChild(mark)
      if (active && !activeMark) activeMark = mark
      cursor = to
    }
    if (cursor < text.length) fragment.appendChild(doc.createTextNode(text.slice(cursor)))
    node.parentNode?.replaceChild(fragment, node)
  }

  return { hits, activeMark }
}
