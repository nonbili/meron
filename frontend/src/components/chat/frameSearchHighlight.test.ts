import { describe, expect, it } from 'bun:test'
import { matchRanges } from './frameSearchHighlight'

// The DOM half of frameSearchHighlight needs a real document (the bun test env
// has none); these cover the offset arithmetic that decides what gets wrapped.
describe('matchRanges', () => {
  it('finds a match inside a single text', () => {
    const { hits, ranges } = matchRanges(['the Quick brown fox'], 'quick')
    expect(hits).toBe(1)
    expect([...ranges]).toEqual([[0, [[4, 9, 0]]]])
  })

  it('spans texts, so a query crossing markup still highlights', () => {
    // <span>Hello</span> <strong>world</strong>
    const { hits, ranges } = matchRanges(['Hello', ' ', 'world'], 'hello world')
    expect(hits).toBe(1)
    expect([...ranges]).toEqual([
      [0, [[0, 5, 0]]],
      [1, [[0, 1, 0]]],
      [2, [[0, 5, 0]]],
    ])
  })

  it('marks every occurrence, including two in one text', () => {
    const { hits, ranges } = matchRanges(['ab ab', 'x', 'AB'], 'ab')
    expect(hits).toBe(3)
    expect([...ranges]).toEqual([
      [
        0,
        [
          [0, 2, 0],
          [3, 5, 1],
        ],
      ],
      [2, [[0, 2, 2]]],
    ])
  })

  it('does not run matches together or wrap past the query', () => {
    const { hits, ranges } = matchRanges(['aaaa'], 'aa')
    expect(hits).toBe(2)
    expect([...ranges]).toEqual([
      [
        0,
        [
          [0, 2, 0],
          [2, 4, 1],
        ],
      ],
    ])
  })

  it('keeps offsets on characters that grow when lowercased', () => {
    // "İ".toLowerCase() is two code units; folding in place would slide the
    // rest of the text left and highlight the wrong character.
    const { hits, ranges } = matchRanges(['İxİ'], 'X')
    expect(hits).toBe(1)
    expect([...ranges]).toEqual([[0, [[1, 2, 0]]]])
  })

  it('still folds the case of a character that expands', () => {
    // "i" matches the "i" of "İ" — the whole source character is marked.
    const { hits, ranges } = matchRanges(['İstanbul'], 'i')
    expect(hits).toBe(1)
    expect([...ranges]).toEqual([[0, [[0, 1, 0]]]])
  })

  it('folds ordinary case in both directions', () => {
    expect([...matchRanges(['straße'], 'STRAßE').ranges]).toEqual([[0, [[0, 6, 0]]]])
    expect([...matchRanges(['ÉCOLE'], 'école').ranges]).toEqual([[0, [[0, 5, 0]]]])
  })

  it('matches a Greek sigma in either of its lowercase forms', () => {
    // "ΟΣ".toLowerCase() is "ος" (final sigma) while "Σ" alone lowercases to
    // "σ" — both fold to "σ" so the query and the body agree either way.
    expect([...matchRanges(['ΟΣ'], 'ΟΣ').ranges]).toEqual([[0, [[0, 2, 0]]]])
    expect([...matchRanges(['ΟΣ'], 'ος').ranges]).toEqual([[0, [[0, 2, 0]]]])
    expect([...matchRanges(['Ος'], 'ΟΣ').ranges]).toEqual([[0, [[0, 2, 0]]]])
  })

  it('reports nothing for an empty query or no match', () => {
    expect(matchRanges(['hello'], '').hits).toBe(0)
    expect(matchRanges(['hello'], 'bye').ranges.size).toBe(0)
  })
})
