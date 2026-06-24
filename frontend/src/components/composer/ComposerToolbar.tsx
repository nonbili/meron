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
import type { Editor } from '@tiptap/react'
import { useTranslation } from '../../lib/i18n'

function ToolbarButton({
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
export function ComposerToolbar({ editor, onSetLink }: { editor: Editor; onSetLink: () => void }) {
  const { t } = useTranslation()

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-0.5 border-b border-border bg-header px-3 py-1.5 select-none">
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
      <span
        className="ml-auto hidden items-center gap-1 pr-1 text-[10px] font-medium text-secondary/70 select-none min-[900px]:flex"
        title={t('composer.toolbar.markdownHint')}
      >
        {t('composer.toolbar.markdown')}:
        <code className="rounded bg-black/[0.06] px-1 dark:bg-white/[0.08]">**bold**</code>
        <code className="rounded bg-black/[0.06] px-1 dark:bg-white/[0.08]"># heading</code>
        <code className="rounded bg-black/[0.06] px-1 dark:bg-white/[0.08]">- list</code>
      </span>
    </div>
  )
}
