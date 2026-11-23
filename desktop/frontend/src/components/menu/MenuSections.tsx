import { Children, Fragment, isValidElement } from 'react'
import type { ReactElement, ReactNode } from 'react'

/** One group of a menu's items, divided from its neighbours by MenuSections. */
export function MenuSection({ children }: { children?: ReactNode }) {
  return <>{children}</>
}

/** The line between two groups of menu items. */
export function MenuDivider() {
  return <div role="separator" className="my-1 h-px bg-border" />
}

/**
 * A menu's items in groups, a line between each two. Many items only show for
 * some messages or folders, so a group can come up empty: it is left out, and
 * no line is drawn for it, so a menu never starts, ends or doubles up on one.
 */
export function MenuSections({ children }: { children: ReactNode }) {
  const sections = Children.toArray(children).filter(
    (child): child is ReactElement<{ children?: ReactNode }> =>
      isValidElement<{ children?: ReactNode }>(child) && Children.toArray(child.props.children).length > 0,
  )
  return (
    <>
      {sections.map((section, at) => (
        <Fragment key={section.key ?? at}>
          {at > 0 && <MenuDivider />}
          {section}
        </Fragment>
      ))}
    </>
  )
}
