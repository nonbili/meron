import { describe, expect, it } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MenuSection, MenuSections } from './MenuSections'

const lines = (html: string) => html.match(/role="separator"/g)?.length ?? 0

describe('MenuSections', () => {
  it('draws a line between each two groups', () => {
    const html = renderToStaticMarkup(
      <MenuSections>
        <MenuSection>
          <button>Open</button>
        </MenuSection>
        <MenuSection>
          <button>Star</button>
          <button>Reply</button>
        </MenuSection>
        <MenuSection>
          <button>Delete</button>
        </MenuSection>
      </MenuSections>,
    )
    expect(lines(html)).toBe(2)
    expect(html.indexOf('Open')).toBeLessThan(html.indexOf('role="separator"'))
  })

  it('leaves out groups whose items are all hidden, and their lines', () => {
    const hidden = false
    const html = renderToStaticMarkup(
      <MenuSections>
        <MenuSection>{hidden && <button>Select</button>}</MenuSection>
        <MenuSection>
          <button>Star</button>
        </MenuSection>
        <MenuSection>
          {hidden && <button>Reply</button>}
          {null}
        </MenuSection>
        <MenuSection>
          <button>Delete</button>
        </MenuSection>
        <MenuSection />
      </MenuSections>,
    )
    expect(lines(html)).toBe(1)
    expect(html.startsWith('<div role="separator"')).toBe(false)
    expect(html.endsWith('</div>')).toBe(false)
  })
})
