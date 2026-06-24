import { useRef, useState } from 'react'
import { ReactNodeViewRenderer, NodeViewWrapper } from '@tiptap/react'
import { Image as TiptapImage } from '@tiptap/extension-image'
import { useTranslation } from '../../lib/i18n'

function ImageNodeView({ node, updateAttributes, selected }: any) {
  const { t } = useTranslation()
  const imgRef = useRef<HTMLImageElement>(null)
  const [resizing, setResizing] = useState(false)

  const handleMouseDown = (e: React.MouseEvent, corner: 'tl' | 'tr' | 'bl' | 'br') => {
    e.preventDefault()
    setResizing(true)

    const startX = e.clientX
    const startWidth = imgRef.current?.offsetWidth || node.attrs.width || 300

    const handleMouseMove = (moveEvent: MouseEvent) => {
      const deltaX = moveEvent.clientX - startX
      let newWidth = startWidth
      if (corner === 'tr' || corner === 'br') {
        newWidth = startWidth + deltaX
      } else {
        newWidth = startWidth - deltaX
      }
      newWidth = Math.max(50, newWidth)
      updateAttributes({ width: `${newWidth}px` })
    }

    const handleMouseUp = () => {
      setResizing(false)
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }

    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)
  }

  const src = node.attrs.src
  const alt = node.attrs.alt
  const width = node.attrs.width
  const height = node.attrs.height

  return (
    <NodeViewWrapper className="relative inline-block my-2 max-w-full">
      <div
        className={`relative inline-block rounded-lg group overflow-hidden ${
          selected ? 'ring-2 ring-accent shadow-lg' : ''
        }`}
      >
        <img
          ref={imgRef}
          src={src}
          alt={alt}
          width={width}
          height={height}
          className="block max-w-full h-auto rounded-lg select-none"
          style={{ width: width || '100%', height: height || 'auto' }}
        />
        {/* Resize Handles (4 Corners) */}
        <div
          onMouseDown={(e) => handleMouseDown(e, 'tl')}
          className={`absolute top-1.5 left-1.5 h-2.5 w-2.5 rounded bg-accent border border-white cursor-nwse-resize shadow-md transition-all ${
            resizing ? 'scale-110 bg-accent/90' : 'opacity-0 group-hover:opacity-100'
          }`}
          title={t('composer.imageResize.topLeft')}
        />
        <div
          onMouseDown={(e) => handleMouseDown(e, 'tr')}
          className={`absolute top-1.5 right-1.5 h-2.5 w-2.5 rounded bg-accent border border-white cursor-nesw-resize shadow-md transition-all ${
            resizing ? 'scale-110 bg-accent/90' : 'opacity-0 group-hover:opacity-100'
          }`}
          title={t('composer.imageResize.topRight')}
        />
        <div
          onMouseDown={(e) => handleMouseDown(e, 'bl')}
          className={`absolute bottom-1.5 left-1.5 h-2.5 w-2.5 rounded bg-accent border border-white cursor-nesw-resize shadow-md transition-all ${
            resizing ? 'scale-110 bg-accent/90' : 'opacity-0 group-hover:opacity-100'
          }`}
          title={t('composer.imageResize.bottomLeft')}
        />
        <div
          onMouseDown={(e) => handleMouseDown(e, 'br')}
          className={`absolute bottom-1.5 right-1.5 h-2.5 w-2.5 rounded bg-accent border border-white cursor-nwse-resize shadow-md transition-all ${
            resizing ? 'scale-110 bg-accent/90' : 'opacity-0 group-hover:opacity-100'
          }`}
          title={t('composer.imageResize.bottomRight')}
        />
      </div>
    </NodeViewWrapper>
  )
}

export const ResizableImage = TiptapImage.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      width: {
        default: null,
        parseHTML: (element) => element.getAttribute('width'),
        renderHTML: (attributes) => {
          if (!attributes.width) return {}
          return { width: attributes.width }
        },
      },
      height: {
        default: null,
        parseHTML: (element) => element.getAttribute('height'),
        renderHTML: (attributes) => {
          if (!attributes.height) return {}
          return { height: attributes.height }
        },
      },
    }
  },
  addNodeView() {
    return ReactNodeViewRenderer(ImageNodeView)
  },
})
