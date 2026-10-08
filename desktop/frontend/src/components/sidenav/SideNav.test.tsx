import { afterEach, expect, it } from 'bun:test'
import { cleanup, render } from '@testing-library/react'
import { SideNav } from './SideNav'

afterEach(cleanup)

// The rail's tiles and selection indicator are sized in rem, so they grow with
// the Text size setting (the root font size). A px rail stays put around them:
// at 120% the 2.75rem tiles leave less room per side than the indicator is wide,
// and the two touch. The rail has to scale with them.
it('sizes the rail in rem, like the tiles and indicator inside it', () => {
  const view = render(<SideNav />)
  const rail = view.container.querySelector('aside')
  expect(rail).not.toBeNull()
  // happy-dom has no layout, so check the class: a spacing-scale width (w-15)
  // or an arbitrary rem one both scale; a px width is what must not come back.
  const width = rail!.className.split(/\s+/).find((name) => name.startsWith('w-'))
  expect(width).toMatch(/^w-(\d+(\.\d+)?|\[[0-9.]+rem\])$/)
})
