import { useState, type ReactNode } from 'react'
import { ExternalLink, Link2 } from 'lucide-react'

import { useTranslation } from '../../lib/i18n'
import { findBareUrls } from '../../lib/linkify'
import { copyText, openExternal } from '../../lib/native'
import { FloatingContextMenu } from '../menu/FloatingContextMenu'
import { MenuItem } from '../menu/MenuItem'

/** Plain text with its bare URLs as links that open in the browser, and a
 *  right-click menu to open or copy one, as in a message. A click on a link
 *  stops there, so a container that opens an editor on click is left alone. */
export function LinkifiedText({ text }: { text: string }) {
  const { t } = useTranslation()
  const [menu, setMenu] = useState<{ x: number; y: number; href: string } | null>(null)
  const parts: ReactNode[] = []
  let copied = 0
  for (const url of findBareUrls(text)) {
    if (url.start > copied) parts.push(text.slice(copied, url.start))
    parts.push(
      <a
        key={url.start}
        href={url.href}
        title={url.href}
        draggable={false}
        onClick={(event) => {
          event.preventDefault()
          event.stopPropagation()
          openExternal(url.href)
        }}
        onContextMenu={(event) => {
          event.preventDefault()
          event.stopPropagation()
          setMenu({ x: event.clientX, y: event.clientY, href: url.href })
        }}
        className="break-all text-accent hover:underline"
      >
        {url.text}
      </a>,
    )
    copied = url.end
  }
  if (copied < text.length) parts.push(text.slice(copied))
  return (
    <>
      {parts}
      {menu && (
        <FloatingContextMenu
          x={menu.x}
          y={menu.y}
          offset={4}
          onClose={() => setMenu(null)}
          overlay
          overlayClassName="fixed inset-0 z-[60]"
          className="fixed z-[61] min-w-[180px]"
          // Keep a press on the menu from starting a task row drag.
          dataAttribute="data-no-drag"
          // The menu is portaled, but React still bubbles its events to the
          // text's container, which may open an editor on click.
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => {
            event.preventDefault()
            event.stopPropagation()
          }}
        >
          <MenuItem
            icon={<ExternalLink size={13} className="text-accent" />}
            label={t('chat.actions.openLink')}
            onClick={() => {
              openExternal(menu.href)
              setMenu(null)
            }}
          />
          <MenuItem
            icon={<Link2 size={13} className="text-accent" />}
            label={t('chat.actions.copyLinkAddress')}
            onClick={() => {
              copyText(menu.href).catch(() => undefined)
              setMenu(null)
            }}
          />
        </FloatingContextMenu>
      )}
    </>
  )
}
