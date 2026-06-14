import { useLayoutEffect, useRef, type MouseEvent, type ReactNode } from 'react'
import { createPortal } from 'react-dom'

export function FloatingContextMenu({
  x,
  y,
  onClose,
  children,
  className,
  offset = 0,
  margin = 8,
  overlay = false,
  overlayClassName = 'fixed inset-0 z-40',
  dataAttribute,
  onClick,
  onContextMenu,
}: {
  x: number
  y: number
  onClose?: () => void
  children: ReactNode
  className: string
  offset?: number
  margin?: number
  overlay?: boolean
  overlayClassName?: string
  dataAttribute?: string
  onClick?: (event: MouseEvent<HTMLDivElement>) => void
  onContextMenu?: (event: MouseEvent<HTMLDivElement>) => void
}) {
  const menuRef = useRef<HTMLDivElement | null>(null)
  const dataProps = dataAttribute ? { [dataAttribute]: '' } : undefined

  useLayoutEffect(() => {
    const el = menuRef.current
    if (!el) return

    const { width, height } = el.getBoundingClientRect()
    const desiredLeft = x + offset
    const desiredTop = y + offset
    const left = Math.max(margin, Math.min(desiredLeft, window.innerWidth - width - margin))
    const top = Math.max(margin, Math.min(desiredTop, window.innerHeight - height - margin))

    el.style.left = `${left}px`
    el.style.top = `${top}px`
  })

  const content = (
    <>
      {overlay && onClose && (
        <div
          className={overlayClassName}
          onClick={onClose}
          onContextMenu={(event) => {
            event.preventDefault()
            onClose()
          }}
        />
      )}
      <div
        ref={menuRef}
        className={className}
        style={{ left: x + offset, top: y + offset }}
        onClick={onClick}
        onContextMenu={onContextMenu}
        {...dataProps}
      >
        {children}
      </div>
    </>
  )

  if (typeof document === 'undefined') return content
  return createPortal(content, document.body)
}
