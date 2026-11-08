import { useEffect, useRef, type ComponentProps, type ReactNode, type RefObject } from 'react'
import { createPortal } from 'react-dom'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { clsx } from '../../lib/utils'

const focusableSelector = 'button, input, select, textarea, a[href], [tabindex], [contenteditable="true"]'

type FocusTarget = { element: HTMLElement | null; owner?: HTMLElement | null; fallback?: FocusTarget }
// A replacement dialog renders before its predecessor restores focus. Retain
// that dialog's return target too, in case its focused control is removed.
const restoreTargets = new WeakMap<HTMLElement, FocusTarget>()

function captureFocus(): FocusTarget {
  const element = document.activeElement as HTMLElement | null
  const dialog = element?.closest<HTMLElement>('[data-dialog-surface]')
  return { element, owner: dialog, fallback: dialog ? restoreTargets.get(dialog) : undefined }
}

function focusableElements(root: HTMLElement) {
  return Array.from(root.querySelectorAll<HTMLElement>(focusableSelector)).filter((element) => {
    if (
      element.tabIndex < 0 ||
      element.matches(':disabled, input[type="hidden"]') ||
      element.closest('[hidden], [inert], [aria-hidden="true"]')
    )
      return false
    for (let ancestor: HTMLElement | null = element; ancestor; ancestor = ancestor.parentElement) {
      const style = getComputedStyle(ancestor)
      if (style.display === 'none' || style.visibility === 'hidden') return false
      if (ancestor === root) break
    }
    return true
  })
}

type DialogProps = Omit<ComponentProps<'div'>, 'onClose' | 'children'> & {
  children: ReactNode
  onClose: () => void
  /** Busy dialogs still consume Escape, without dismissing the layer below. */
  dismissible?: boolean
  closeOnBackdrop?: boolean
  onEscape?: () => void
  backdropClassName?: string
  initialFocusRef?: RefObject<HTMLElement | null>
  /** Keep the backdrop and centering within a positioned owner, such as a composer. */
  portalContainer?: HTMLElement
  role?: 'dialog' | 'alertdialog'
}

/** Shared modal shell. Surfaces and backdrops deliberately have no entrance animation. */
export function Dialog({
  children,
  onClose,
  dismissible = true,
  closeOnBackdrop = true,
  onEscape = onClose,
  backdropClassName,
  className,
  initialFocusRef,
  portalContainer,
  role = 'dialog',
  ...props
}: DialogProps) {
  const panel = useRef<HTMLDivElement>(null)
  const previousFocus = useRef(captureFocus())
  useEscapeKey(() => {
    if (dismissible) onEscape()
  })

  useEffect(() => {
    const root = panel.current
    if (!root) return
    const previous = previousFocus.current
    restoreTargets.set(root, previous)
    if (initialFocusRef?.current) initialFocusRef.current.focus()
    else if (!root.contains(document.activeElement)) (focusableElements(root)[0] ?? root).focus()

    // Listen on the document so portalled menus remain usable. Only the topmost
    // modal handles Tab; menus inside it may take focus outside its DOM subtree.
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Tab' || event.defaultPrevented) return
      const dialogs = Array.from(document.querySelectorAll<HTMLElement>('[data-dialog-backdrop]'))
      dialogs.sort((a, b) => Number(getComputedStyle(a).zIndex) - Number(getComputedStyle(b).zIndex))
      if (dialogs.at(-1) !== root.parentElement) return
      const items = focusableElements(root)
      const first = items[0] ?? root
      const last = items.at(-1) ?? root
      if (!root.contains(document.activeElement) || document.activeElement === root) {
        event.preventDefault()
        ;(event.shiftKey ? last : first).focus()
      } else if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('keydown', onKeyDown)
      if (root.contains(document.activeElement) || document.activeElement === document.body) {
        let target: FocusTarget | undefined = previous
        while (target && !target.element?.isConnected) {
          // A confirmation can remove its opener without closing its parent.
          // Keep focus inside that surviving modal before considering ancestors.
          if (target.owner?.isConnected) {
            ;(focusableElements(target.owner)[0] ?? target.owner).focus()
            return
          }
          target = target.fallback
        }
        target?.element?.focus()
      }
    }
  }, [initialFocusRef])

  return createPortal(
    <div
      data-dialog-backdrop=""
      className={clsx('dialog-backdrop', backdropClassName)}
      style={portalContainer ? { position: 'absolute' } : undefined}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && closeOnBackdrop && dismissible) onClose()
      }}
    >
      <div
        {...props}
        ref={panel}
        data-dialog-surface=""
        role={role}
        aria-modal="true"
        tabIndex={-1}
        className={clsx('dialog-surface', className)}
      >
        {children}
      </div>
    </div>,
    portalContainer ?? document.body,
  )
}
