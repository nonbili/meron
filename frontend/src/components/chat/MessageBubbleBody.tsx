import { Copy } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { openExternal } from '../../lib/native'
import type { Message } from '../../types'
import {
  MESSAGE_BODY_MAX_HEIGHT,
  escapeRegExp,
  getShortenedLinkText,
  normalizeBodyText,
  parseInlineMessageContent,
  splitFencedCodeBlocks,
} from './messageHelpers'
import { BubbleHtmlFrame } from './BubbleHtmlFrame'

// The message body: the sandboxed HTML view, or the plain/markdown renderer with
// inline bold/italic/code, fenced code blocks (with copy buttons) and links,
// highlighting the in-thread search query when one is active.
export function MessageBubbleBody({
  message,
  useHtmlBody,
  normalizedSearchQuery,
  activeSearchMatch,
}: {
  message: Message
  useHtmlBody: boolean
  normalizedSearchQuery: string
  activeSearchMatch: boolean
}) {
  const { t } = useTranslation()
  if (useHtmlBody) {
    return (
      <div className="relative overflow-y-auto -mr-3.5 pr-3.5" style={{ maxHeight: MESSAGE_BODY_MAX_HEIGHT }}>
        <BubbleHtmlFrame html={message.body_html!} />
      </div>
    )
  }

  const bodyText = normalizeBodyText(message.body)
  const hasCodeFence = bodyText.split('\n').some((line) => line.trimStart().startsWith('```'))
  const blocks = hasCodeFence
    ? splitFencedCodeBlocks(bodyText)
    : [{ type: 'inline' as const, parts: parseInlineMessageContent(bodyText) }]

  function renderHighlightedPlainText(content: string, keyPrefix: string) {
    if (!normalizedSearchQuery) return content
    const regex = new RegExp(`(${escapeRegExp(normalizedSearchQuery)})`, 'ig')
    return content.split(regex).map((chunk, index) => {
      if (chunk.toLowerCase() !== normalizedSearchQuery.toLowerCase()) return chunk
      return (
        <mark
          key={`${keyPrefix}-match-${index}`}
          className={`rounded px-0.5 ${
            activeSearchMatch ? 'bg-amber-300 text-black' : 'bg-amber-200/70 text-inherit dark:bg-amber-400/35'
          }`}
        >
          {chunk}
        </mark>
      )
    })
  }

  function renderText(content: string, keyPrefix: string) {
    const chunks = content.split(/(`[^`\n]+`|\*\*[^*]+\*\*|\*[^*\n]+\*)/g)
    return chunks.map((chunk, index) => {
      const bold = chunk.match(/^\*\*([^*]+)\*\*$/)
      if (bold) {
        return (
          <strong key={`${keyPrefix}-${index}`} className="font-semibold">
            {renderHighlightedPlainText(bold[1], `${keyPrefix}-bold-${index}`)}
          </strong>
        )
      }
      const italic = chunk.match(/^\*([^*\n]+)\*$/)
      if (italic) {
        return (
          <em key={`${keyPrefix}-${index}`}>{renderHighlightedPlainText(italic[1], `${keyPrefix}-italic-${index}`)}</em>
        )
      }
      const inlineCode = chunk.match(/^`([^`\n]+)`$/)
      if (inlineCode) {
        return (
          <code
            key={`${keyPrefix}-${index}`}
            className="rounded bg-black/5 px-1 py-0.5 font-mono text-[0.9em] text-primary dark:bg-white/10"
          >
            {renderHighlightedPlainText(inlineCode[1], `${keyPrefix}-code-${index}`)}
          </code>
        )
      }
      return renderHighlightedPlainText(chunk, `${keyPrefix}-${index}`)
    })
  }

  return (
    <div
      className="relative overflow-y-auto -mr-3.5 pr-3.5 text-[15px] leading-relaxed break-words whitespace-pre-wrap select-text font-normal tracking-[0.01em]"
      style={{ maxHeight: MESSAGE_BODY_MAX_HEIGHT }}
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
              <pre className="m-0 max-w-full overflow-x-auto rounded-lg border border-border/60 bg-black/5 px-3 py-2.5 pr-11 pb-2 font-mono text-[13px] leading-relaxed text-primary shadow-inner dark:bg-white/10">
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
