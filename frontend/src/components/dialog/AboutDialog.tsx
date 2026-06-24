import { useEffect } from 'react'
import { ExternalLink, Heart, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { ui$ } from '../../states/ui'
import { openExternal } from '../../lib/native'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import logo from '../../assets/logo.png'
import wailsConfig from '../../../../wails.json'

const DONATE_LINKS = [
  { label: 'GitHub', url: 'https://github.com/sponsors/nonbili' },
  { label: 'Liberapay', url: 'https://liberapay.com/rnons' },
  { label: 'PayPal', url: 'https://paypal.me/nonbili' },
]

export function AboutDialog() {
  const { t } = useTranslation()
  const open = useValue(ui$.aboutOpen)

  const onClose = () => ui$.aboutOpen.set(false)

  useEffect(() => {
    if (!open) return
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        onClose()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [open])

  if (!open) return null

  const productName = wailsConfig.info.productName
  const version = wailsConfig.info.productVersion
  const comments = wailsConfig.info.comments

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/35 p-4 backdrop-blur-[3px] dark:bg-black/60"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose()
      }}
    >
      <div
        className="flex w-full max-w-sm flex-col overflow-hidden rounded-3xl border border-border bg-chats text-primary shadow-2xl shadow-black/20 animate-slide-up dark:shadow-black/45"
        role="dialog"
        aria-modal="true"
        aria-label={t('about.aboutProduct', { product: productName })}
      >
        <div className="flex items-center justify-between border-b border-border/70 px-5 py-3.5">
          <h2 className="text-sm font-bold tracking-tight">{t('about.aboutProduct', { product: productName })}</h2>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>

        <div className="flex flex-col items-center px-6 py-7 text-center">
          <img src={logo} alt="" className="h-20 w-20 object-contain" />
          <h3 className="mt-4 text-xl font-bold tracking-tight">{productName}</h3>
          <p className="mt-1 text-xs font-semibold text-secondary">{t('about.version', { version })}</p>
          <p className="mt-4 max-w-[18rem] text-sm leading-6 text-secondary">{comments}</p>

          <div className="mt-6 w-full rounded-2xl border border-border/70 bg-raised/70 p-4">
            <div className="flex items-center justify-center gap-2 text-xs font-bold text-primary">
              <Heart size={14} className="text-accent" />
              <span>{t('about.supportDevelopment')}</span>
            </div>
            <div className="mt-3 grid grid-cols-3 gap-2">
              {DONATE_LINKS.map((link) => (
                <Button
                  key={link.url}
                  variant="secondary"
                  size="sm"
                  rightIcon={ExternalLink}
                  className="px-2 text-[10px]"
                  onClick={() => openExternal(link.url)}
                >
                  {link.label}
                </Button>
              ))}
            </div>
          </div>
        </div>

        <div className="flex justify-end border-t border-border/70 px-5 py-4">
          <Button variant="secondary" onClick={onClose}>
            {t('buttons.close')}
          </Button>
        </div>
      </div>
    </div>
  )
}
