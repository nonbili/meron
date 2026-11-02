import { describe, expect, it } from 'bun:test'
import { threadListFolderLabelClass } from './ThreadList'

describe('threadListFolderLabelClass', () => {
  // With the search in the title bar the header has no search box to protect,
  // so a fixed share cut off short names ("Unified inbox") with a larger Text size.
  it('lets the folder name take the room it needs when the search is in the title bar', () => {
    const classes = threadListFolderLabelClass(true).split(/\s+/)
    expect(classes.some((name) => name.startsWith('max-w-'))).toBe(false)
    expect(classes).toContain('min-w-0')
    expect(classes).not.toContain('shrink-0')
  })

  it('keeps the cap that leaves room for the search box in the header', () => {
    expect(threadListFolderLabelClass(false).split(/\s+/)).toContain('max-w-[40%]')
  })
})
