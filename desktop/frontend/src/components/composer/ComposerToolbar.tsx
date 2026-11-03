import {
  Bold,
  Italic,
  Underline as UnderlineIcon,
  Strikethrough,
  Heading2,
  List,
  ListOrdered,
  Quote,
  Link2,
} from 'lucide-react'
import { useRef, useState } from 'react'
import type { Editor } from '@tiptap/react'
import { useTranslation } from '../../lib/i18n'
import { MenuSurface } from '../menu/MenuSurface'
import { useDismissOnOutside } from '../menu/useDismissOnOutside'
import { MarkdownIcon } from '../icons/Icons'

// Shortcuts the editor turns into formatting as you type (StarterKit's input
// rules). Syntax, not words, so it needs no translation.
const MARKDOWN_EXAMPLES = ['**bold**', '*italic*', '~~strike~~', '# heading', '- list', '1. list', '> quote']

// Behind an icon rather than inline: spelled out, the examples wrapped the
// toolbar onto a second row whenever the composer sat in a narrow pane.
function MarkdownHelp() {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  useDismissOnOutside(
    open,
    (target) => Boolean(rootRef.current?.contains(target as Node)),
    () => setOpen(false),
  )

  return (
    // preventDefault keeps focus in the editor. Unlike the formatting buttons,
    // this one never hands it back, and with it gone the next keys typed would
    // run the app's single-key shortcuts, archiving or deleting the thread.
    <div ref={rootRef} className="relative" onMouseDown={(event) => event.preventDefault()}>
      <ToolbarButton active={open} onClick={() => setOpen((v) => !v)} title={t('composer.toolbar.markdown')}>
        <MarkdownIcon size={17} />
      </ToolbarButton>
      {open && (
        <MenuSurface role="dialog" className="absolute right-0 top-full z-50 mt-1 w-max max-w-[16rem] p-3">
          <p className="mb-2 text-[0.6875rem] leading-snug text-secondary">{t('composer.toolbar.markdownHint')}</p>
          <div className="flex flex-wrap gap-1">
            {MARKDOWN_EXAMPLES.map((example) => (
              <code
                key={example}
                className="rounded bg-black/[0.06] px-1 text-[0.6875rem] text-primary dark:bg-white/[0.08]"
              >
                {example}
              </code>
            ))}
          </div>
        </MenuSurface>
      )}
    </div>
  )
}

export function ToolbarButton({
  active,
  onClick,
  title,
  children,
}: {
  active: boolean
  onClick: () => void
  title: string
  children: React.ReactNode
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`flex h-7 w-7 items-center justify-center rounded-lg transition-colors cursor-pointer ${
        active ? 'bg-accent/15 text-accent' : 'text-secondary hover:bg-hover'
      }`}
    >
      {children}
    </button>
  )
}

// Rich-text formatting toolbar shown above the body in rich mode.
export function ComposerToolbar({
  editor,
  onSetLink,
  trailing,
}: {
  editor: Editor
  onSetLink: () => void
  trailing?: React.ReactNode
}) {
  const { t } = useTranslation()

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-0.5 border-b border-border/60 bg-chat px-3 py-1.5 select-none">
      <ToolbarButton
        active={editor.isActive('bold')}
        onClick={() => editor.chain().focus().toggleBold().run()}
        title={t('composer.toolbar.bold')}
      >
        <Bold size={15} />
      </ToolbarButton>
      <ToolbarButton
        active={editor.isActive('italic')}
        onClick={() => editor.chain().focus().toggleItalic().run()}
        title={t('composer.toolbar.italic')}
      >
        <Italic size={15} />
      </ToolbarButton>
      <ToolbarButton
        active={editor.isActive('underline')}
        onClick={() => editor.chain().focus().toggleUnderline().run()}
        title={t('composer.toolbar.underline')}
      >
        <UnderlineIcon size={15} />
      </ToolbarButton>
      <ToolbarButton
        active={editor.isActive('strike')}
        onClick={() => editor.chain().focus().toggleStrike().run()}
        title={t('composer.toolbar.strikethrough')}
      >
        <Strikethrough size={15} />
      </ToolbarButton>
      <span className="mx-1 h-4 w-px bg-border" />
      <ToolbarButton
        active={editor.isActive('heading', { level: 2 })}
        onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
        title={t('composer.toolbar.heading')}
      >
        <Heading2 size={15} />
      </ToolbarButton>
      <ToolbarButton
        active={editor.isActive('bulletList')}
        onClick={() => editor.chain().focus().toggleBulletList().run()}
        title={t('composer.toolbar.bulletList')}
      >
        <List size={15} />
      </ToolbarButton>
      <ToolbarButton
        active={editor.isActive('orderedList')}
        onClick={() => editor.chain().focus().toggleOrderedList().run()}
        title={t('composer.toolbar.numberedList')}
      >
        <ListOrdered size={15} />
      </ToolbarButton>
      <ToolbarButton
        active={editor.isActive('blockquote')}
        onClick={() => editor.chain().focus().toggleBlockquote().run()}
        title={t('composer.toolbar.quote')}
      >
        <Quote size={15} />
      </ToolbarButton>
      <ToolbarButton active={editor.isActive('link')} onClick={onSetLink} title={t('composer.toolbar.link')}>
        <Link2 size={15} />
      </ToolbarButton>
      <div className="ml-auto flex items-center">
        <MarkdownHelp />
        {trailing}
      </div>
    </div>
  )
}
