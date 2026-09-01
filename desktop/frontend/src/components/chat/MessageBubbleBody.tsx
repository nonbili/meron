import { useEffect, useRef } from 'react'
import type { ReactNode } from 'react'
import { Copy } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { openExternal } from '../../lib/native'
import type { Message } from '../../types'
import {
  MESSAGE_BODY_MAX_HEIGHT,
  getShortenedLinkText,
  messageContentBlocks,
  normalizeBodyText,
  splitInlineMarkup,
} from './messageHelpers'
import { BubbleHtmlFrame } from './BubbleHtmlFrame'
import { matchRanges } from './frameSearchHighlight'

// The message body: the sandboxed HTML view, or the plain/markdown renderer with
// inline bold/italic/code, fenced code blocks (with copy buttons) and links,
// highlighting the in-thread search query when one is active.
export function MessageBubbleBody({
  message,
  useHtmlBody,
  outgoing = false,
  allowRemote = false,
  normalizedSearchQuery,
  activeSearchOffset,
  fullHeight = false,
  onLinkHover,
  onUserScrollIntent,
}: {
  message: Message
  useHtmlBody: boolean
  /** Whether this message sits in an outgoing bubble; its HTML frame is painted
   *  with that bubble's colors. */
  outgoing?: boolean
  /** Whether this message's remote content may load (account setting, allowed
   *  sender, or a reveal the user just made). */
  allowRemote?: boolean
  normalizedSearchQuery: string
  /** Which of this message's matches the search is parked on, -1 for none:
   *  that one gets the stronger highlight and is what the pane scrolls to. */
  activeSearchOffset: number
  /** Grow to fit the content instead of scrolling inside a capped box — the
   *  traditional layout lets the conversation itself do the scrolling. */
  fullHeight?: boolean
  onLinkHover?: (url: string | null) => void
  onUserScrollIntent?: () => void
}) {
  const { t } = useTranslation()
  const bodyRef = useRef<HTMLDivElement | null>(null)
  // The active <mark> is scrolled to from here, where it is rendered — the pane
  // only knows which message to bring into view, and its own effect runs a
  // render before this body has moved its highlight. (The HTML frame does the
  // same for the marks it places inside its document.)
  useEffect(() => {
    if (activeSearchOffset < 0) return
    const mark = bodyRef.current?.querySelector<HTMLElement>('[data-search-active="true"]')
    mark?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }, [activeSearchOffset, normalizedSearchQuery, message.id])

  const boxStyle = fullHeight ? undefined : { maxHeight: MESSAGE_BODY_MAX_HEIGHT }
  const boxClass = fullHeight ? 'relative' : 'relative overflow-y-auto'
  if (useHtmlBody) {
    return (
      <div className={`${boxClass} -mr-3.5 pr-3.5`} style={boxStyle}>
        <BubbleHtmlFrame
          html={message.body_html!}
          outgoing={outgoing}
          allowRemote={allowRemote}
          searchQuery={normalizedSearchQuery}
          activeSearchOffset={activeSearchOffset}
          onLinkHover={onLinkHover}
          onUserScrollIntent={onUserScrollIntent}
        />
      </div>
    )
  }

  const bodyText = normalizeBodyText(message.body)
  const blocks = messageContentBlocks(bodyText)
  // Matches are numbered as they are rendered, in the same order the search bar
  // counted them (plainHighlightTexts walks these blocks), so occurrence n here
  // is occurrence n there. Reset on every render pass.
  let matchOrdinal = 0

  // matchRanges is the matcher the search bar counted with (and the one the HTML
  // frame marks with), so every match it counted gets a <mark> here — a private
  // regex would disagree with it over case folding: /i/i does not match "İ".
  function renderHighlightedPlainText(content: string, keyPrefix: string): ReactNode {
    if (!normalizedSearchQuery) return content
    const hits = matchRanges([content], normalizedSearchQuery).ranges.get(0)
    if (!hits) return content

    const nodes: ReactNode[] = []
    let cursor = 0
    for (const [from, to] of hits) {
      if (to <= cursor) continue
      const start = Math.max(from, cursor)
      if (start > cursor) nodes.push(content.slice(cursor, start))
      const active = matchOrdinal === activeSearchOffset
      matchOrdinal += 1
      nodes.push(
        <mark
          key={`${keyPrefix}-match-${start}`}
          data-search-active={active ? 'true' : undefined}
          className={`rounded px-0.5 ${
            active ? 'bg-amber-300 text-black' : 'bg-amber-200/70 text-inherit dark:bg-amber-400/35'
          }`}
        >
          {content.slice(start, to)}
        </mark>,
      )
      cursor = to
    }
    if (cursor < content.length) nodes.push(content.slice(cursor))
    return nodes
  }

  function renderText(content: string, keyPrefix: string) {
    return splitInlineMarkup(content).map((chunk, index) => {
      if (chunk.type === 'bold') {
        return (
          <strong key={`${keyPrefix}-${index}`} className="font-semibold">
            {renderHighlightedPlainText(chunk.text, `${keyPrefix}-bold-${index}`)}
          </strong>
        )
      }
      if (chunk.type === 'italic') {
        return (
          <em key={`${keyPrefix}-${index}`}>
            {renderHighlightedPlainText(chunk.text, `${keyPrefix}-italic-${index}`)}
          </em>
        )
      }
      if (chunk.type === 'code') {
        return (
          <code
            key={`${keyPrefix}-${index}`}
            className="rounded bg-black/5 px-1 py-0.5 font-mono text-[0.9em] text-primary dark:bg-white/10"
          >
            {renderHighlightedPlainText(chunk.text, `${keyPrefix}-code-${index}`)}
          </code>
        )
      }
      return renderHighlightedPlainText(chunk.text, `${keyPrefix}-${index}`)
    })
  }

  return (
    <div
      ref={bodyRef}
      className={`${boxClass} -mr-3.5 pr-3.5 font-message text-[calc(0.9375rem*var(--me-message-scale))] leading-relaxed break-words whitespace-pre-wrap select-text font-normal tracking-[0.01em]`}
      style={boxStyle}
    >
      {blocks.map((block, blockIndex) => {
        if (block.type === 'code') {
          return (
            <div key={`code-${blockIndex}`} className="group relative my-2 max-w-full">
              <button
                type="button"
                onClick={() => navigator.clipboard?.writeText(block.content).catch(() => undefined)}
                className="absolute right-1.5 top-1.5 z-10 flex h-7 w-7 items-center justify-center rounded-md border border-border/70 bg-chats/90 text-secondary opacity-0 shadow-sm transition-opacity hover:text-primary group-hover:opacity-100"
                title={t('chat.copyCode')}
              >
                <Copy size={13} />
              </button>
              <pre className="m-0 max-w-full overflow-x-auto rounded-lg border border-border/60 bg-black/5 px-3 py-2.5 pr-11 pb-2 font-mono text-[calc(0.8125rem*var(--me-message-scale))] leading-relaxed text-primary shadow-inner dark:bg-white/10">
                <code className="block min-w-max whitespace-pre">{block.content}</code>
              </pre>
            </div>
          )
        }

        return (
          <span key={`inline-${blockIndex}`}>
            {block.parts.map((part, index) => {
              if (part.type === 'link') {
                return (
                  <a
                    key={index}
                    href={part.content}
                    onClick={(e) => {
                      e.preventDefault()
                      openExternal(part.content)
                    }}
                    onMouseEnter={() => onLinkHover?.(part.content)}
                    onMouseLeave={() => onLinkHover?.(null)}
                    onFocus={() => onLinkHover?.(part.content)}
                    onBlur={() => onLinkHover?.(null)}
                    title={part.content}
                    className="text-accent hover:underline break-all font-semibold cursor-pointer"
                  >
                    {part.label
                      ? renderText(part.label, `link-${blockIndex}-${index}`)
                      : getShortenedLinkText(part.content)}
                  </a>
                )
              }
              return renderText(part.content, `text-${blockIndex}-${index}`)
            })}
          </span>
        )
      })}
    </div>
  )
}
