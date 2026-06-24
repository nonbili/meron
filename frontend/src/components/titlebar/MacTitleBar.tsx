import type { CSSProperties } from 'react'
import { isMac } from '../../lib/shortcuts'

// With the native macOS title bar hidden (see main.go), content reaches the top
// window edge and the traffic-light buttons float over the top-left corner.
// This slim strip reserves that vertical space and provides the draggable
// region the hidden title bar would normally give us. `--wails-draggable` is
// the Wails hook that makes an element move the window.
const DRAG: CSSProperties = { '--wails-draggable': 'drag' } as CSSProperties

export function MacTitleBar() {
  if (!isMac) return null
  return <div className="flex h-7 shrink-0 items-center bg-sidenav" style={DRAG} aria-hidden />
}
