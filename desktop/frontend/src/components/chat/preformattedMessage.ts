export const TRANSPARENT_TRACKING_PIXEL =
  'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7'

const NON_CONTENT = `style, img[width="1"][height="1"], img[src="${TRANSPARENT_TRACKING_PIXEL}"]`
const CJK_SENTENCE =
  /^[\t \u3000]*[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}][^\r\n]*[。！？][\t \u3000]*$/mu

function onlyContentElement(container: HTMLElement): HTMLElement | null {
  let content: HTMLElement | null = null
  for (const node of container.childNodes) {
    if (node.nodeType === Node.TEXT_NODE && node.textContent?.trim()) return null
    if (node.nodeType !== Node.ELEMENT_NODE) continue
    const element = node as HTMLElement
    if (element.matches(NON_CONTENT)) continue
    if (content) return null
    content = element
  }
  return content
}

// Some mailers put Japanese prose in a bare <pre> to preserve indentation.
// Require a sentence so bare code, logs and diagrams keep their line lengths.
export function wrapPreformattedMessage(doc: Document) {
  if (!doc.body) return
  let content = onlyContentElement(doc.body)
  while (content?.tagName === 'DIV') content = onlyContentElement(content)
  if (content?.matches('pre:not([class], [style])') && !content.querySelector('code, samp, kbd')) {
    if (CJK_SENTENCE.test(content.textContent ?? '')) content.style.whiteSpace = 'pre-wrap'
  }
}
