import { useEffect, useRef, useState } from 'react'
import { Check, ChevronRight, Plus, Tag } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { loadTagCounts, openTagDialog, threadTags, toggleThreadTag, type TagCount } from '../../states/mailTags'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'
import { flyoutClass, useFlyoutPosition } from './ThreadMoveCopyItems'

const FLYOUT_WIDTH = 230
const MAX_ROWS = 8
// Long enough to cross the gap to the flyout, whose edge the row doesn't touch.
const CLOSE_DELAY_MS = 200

/**
 * The "Tags" row of a mail thread's menu. Its flyout toggles the thread's tags
 * and the most used others, staying open so several can change at once; adding
 * anything else goes through the tag dialog, as does the row itself when there
 * is nothing to list. Mount it with `key={threadId}`.
 */
export function ThreadTagItem({
  threadId,
  close,
  itemClassName,
  iconSize = 13,
}: {
  threadId: string
  close: () => void
  itemClassName?: string
  iconSize?: number
}) {
  const { t } = useTranslation()
  // Rows stay where they opened, so a toggle never moves one under the cursor.
  const [initial] = useState(() => threadTags(threadId))
  const [tags, setTags] = useState(initial)
  const [known, setKnown] = useState<TagCount[]>([])
  const [open, setOpen] = useState(false)
  const latest = useRef(initial)
  const anchorRef = useRef<HTMLDivElement | null>(null)
  const position = useFlyoutPosition(open, anchorRef, FLYOUT_WIDTH)
  const closing = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)

  useEffect(() => {
    void loadTagCounts().then(setKnown, () => setKnown([]))
    return () => clearTimeout(closing.current)
  }, [])

  const others = known
    .filter(({ tag }) => !initial.includes(tag))
    .sort((a, b) => b.count - a.count)
    .slice(0, Math.max(0, MAX_ROWS - initial.length))
    .map(({ tag }) => tag)
  const rows = [...initial, ...others]

  const openDialog = () => {
    close()
    openTagDialog(threadId)
  }

  const toggle = (tag: string) => {
    void toggleThreadTag(threadId, tag, () => latest.current).then((next) => {
      // Refused, the check mark stays as it was: what the core still holds.
      if (!next) return
      latest.current = next
      setTags(next)
    })
  }

  return (
    <div
      ref={anchorRef}
      onMouseEnter={() => {
        clearTimeout(closing.current)
        setOpen(rows.length > 0)
      }}
      onMouseLeave={() => {
        closing.current = setTimeout(() => setOpen(false), CLOSE_DELAY_MS)
      }}
    >
      <MenuItem
        icon={<Tag size={iconSize} className="shrink-0 text-secondary" />}
        label={t('tags.title')}
        submenuOpen={rows.length > 0 ? open : undefined}
        trailing={rows.length > 0 ? <ChevronRight size={13} className="text-secondary" /> : undefined}
        className={itemClassName}
        onClick={() => (rows.length > 0 ? setOpen(true) : openDialog())}
      />
      {open && position && (
        <FloatingContextMenu
          x={position.x}
          y={position.y}
          className={`${flyoutClass} w-[230px]`}
          dataAttribute="data-thread-context-menu"
        >
          {rows.map((tag) => (
            <MenuItem
              key={tag}
              label={
                <span className="min-w-0 truncate" title={tag}>
                  {tag}
                </span>
              }
              pressed={tags.includes(tag)}
              trailing={tags.includes(tag) ? <Check size={13} className="shrink-0 text-accent" /> : undefined}
              onClick={() => toggle(tag)}
            />
          ))}
          <div className="my-1 border-t border-border" />
          <MenuItem
            icon={<Plus size={13} className="shrink-0 text-secondary" />}
            label={t('tags.placeholder')}
            onClick={openDialog}
          />
        </FloatingContextMenu>
      )}
    </div>
  )
}
