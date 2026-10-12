import { useContext, useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { Copy } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { copyText, openExternal } from '../../lib/native'
import { thread$ } from '../../states/thread'
import type { Message } from '../../types'
import {
  MESSAGE_BODY_MAX_HEIGHT,
  bodyContentKey,
  getShortenedLinkText,
  messageContentBlocks,
  normalizeBodyText,
  plainHighlightTexts,
  splitInlineMarkup,
} from './messageHelpers'
import { BubbleHtmlFrame } from './BubbleHtmlFrame'
import { matchRanges } from './frameSearchHighlight'
import { isQuoteUnfolded, setQuoteUnfolded, splitQuotedBody } from './quoteFold'
import { ConversationScrollIntentContext } from './useConversationScroll'
import type { FrameSurface } from './frameTheme'

const setHoveredLink = (url: string | null) => thread$.hoveredLink.set(url)

// The message body: the sandboxed HTML view, or the plain/markdown renderer with
// inline bold/italic/code, fenced code blocks (with copy buttons) and links,
// highlighting the in-thread search query when one is active.
export function MessageBubbleBody({
  message,
  useHtmlBody,
  outgoing = false,
  surface = 'bubble',
  allowRemote = false,
  normalizedSearchQuery,
  activeSearchOffset,
  fullHeight = false,
  onNaturalWidth,
}: {
  message: Message
  useHtmlBody: boolean
  /** Whether this message sits in an outgoing bubble; its HTML frame is painted
   *  with that bubble's colors. */
  outgoing?: boolean
  /** A chat bubble, a draft's, or a traditional layout's row: what a darkened body blends onto. */
  surface?: FrameSurface
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
  /** HTML body only: the width a plain-text-like document needs, null when it should fill the bubble. */
  onNaturalWidth?: (width: number | null) => void
}) {
  const { t } = useTranslation()
  const bodyRef = useRef<HTMLDivElement | null>(null)
  const releasePinForUserScroll = useContext(ConversationScrollIntentContext)
  // The active <mark> is scrolled to from here, where it is rendered — the pane
  // only knows which message to bring into view, and its own effect runs a
  // render before this body has moved its highlight. (The HTML frame does the
  // same for the marks it places inside its document.)
  useEffect(() => {
    if (activeSearchOffset < 0) return
    const mark = bodyRef.current?.querySelector<HTMLElement>('[data-search-active="true"]')
    mark?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }, [activeSearchOffset, normalizedSearchQuery, message.id])

  // The plain body's quoted tail folds behind a toggle (see quoteFold).
  const quoteKey = bodyContentKey(message.body)
  const [quoteUnfolded, setQuoteUnfoldedState] = useState(() => isQuoteUnfolded(quoteKey))
  useEffect(() => setQuoteUnfoldedState(isQuoteUnfolded(quoteKey)), [quoteKey])

  const boxStyle = fullHeight ? undefined : { maxHeight: MESSAGE_BODY_MAX_HEIGHT }
  const boxClass = fullHeight ? 'relative' : 'relative overflow-y-auto'
  if (useHtmlBody) {
    return (
      <div className={`${boxClass} -mr-3.5 pr-3.5`} style={boxStyle}>
        <BubbleHtmlFrame
          html={message.body_html!}
          outgoing={outgoing}
          surface={surface}
          allowRemote={allowRemote}
          mediaMissing={message.media_missing ?? 0}
          searchQuery={normalizedSearchQuery}
          activeSearchOffset={activeSearchOffset}
          onLinkHover={setHoveredLink}
          // Only a full-height body hands the wheel on to the pane; a capped
          // one scrolls itself and leaves the pane where it is.
          onUserScrollIntent={fullHeight ? releasePinForUserScroll : undefined}
          onNaturalWidth={onNaturalWidth}
        />
      </div>
    )
  }

  const { reply, quote } = splitQuotedBody(message.body, message.body_quote_start)
  // A search parked on a match inside the folded quote opens it: the reply's
  // matches come first in the count, so any later occurrence is in the quote.
  const replyHits =
    quote && activeSearchOffset >= 0
      ? plainHighlightTexts(reply).reduce((sum, text) => sum + matchRanges([text], normalizedSearchQuery).hits, 0)
      : 0
  const showQuote = !!quote && (quoteUnfolded || (activeSearchOffset >= 0 && activeSearchOffset >= replyHits))
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

  // Reply and quote are normalized apart, the way plainHighlightTexts counts them.
  function renderSection(text: string, section: string) {
    return messageContentBlocks(normalizeBodyText(text)).map((block, blockIndex) => {
      if (block.type === 'code') {
        return (
          <div key={`${section}-code-${blockIndex}`} className="group relative my-2 max-w-full">
            <button
              type="button"
              onClick={() => copyText(block.content).catch(() => undefined)}
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
        <span key={`${section}-inline-${blockIndex}`}>
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
                  onMouseEnter={() => setHoveredLink(part.content)}
                  onMouseLeave={() => setHoveredLink(null)}
                  onFocus={() => setHoveredLink(part.content)}
                  onBlur={() => setHoveredLink(null)}
                  title={part.content}
                  className="text-accent hover:underline break-all font-semibold cursor-pointer"
                >
                  {part.label
                    ? renderText(part.label, `${section}-link-${blockIndex}-${index}`)
                    : getShortenedLinkText(part.content)}
                </a>
              )
            }
            return renderText(part.content, `${section}-text-${blockIndex}-${index}`)
          })}
        </span>
      )
    })
  }

  const quoteLabel = showQuote ? t('chat.hideQuotedText') : t('chat.showQuotedText')
  return (
    <div
      ref={bodyRef}
      className={`${boxClass} -mr-3.5 pr-3.5 font-message text-[calc(0.9375rem*var(--me-message-scale))] leading-relaxed break-words whitespace-pre-wrap select-text font-normal tracking-[0.01em]`}
      style={boxStyle}
    >
      {renderSection(reply, 'reply')}
      {quote && (
        <>
          <span className="my-1.5 block">
            <button
              type="button"
              onClick={() => {
                setQuoteUnfolded(quoteKey, !showQuote)
                setQuoteUnfoldedState(!showQuote)
              }}
              title={quoteLabel}
              aria-label={quoteLabel}
              aria-expanded={showQuote}
              className="flex h-3.5 w-7 cursor-pointer items-center justify-center rounded-full border border-border/70 bg-black/5 font-sans text-[10px] leading-none font-bold tracking-[1px] text-secondary hover:bg-black/10 hover:text-primary dark:bg-white/10 dark:hover:bg-white/15 aria-expanded:border-secondary aria-expanded:bg-black/15 aria-expanded:text-primary dark:aria-expanded:bg-white/25"
            >
              •••
            </button>
          </span>
          {showQuote && renderSection(quote, 'quote')}
        </>
      )}
    </div>
  )
}
