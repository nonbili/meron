import { ExternalLink, Heart, ScrollText, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { Dialog } from './Dialog'
import { ui$ } from '../../states/ui'
import { openExternal } from '../../lib/native'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import { UpdateSection } from './UpdateSection'
import logo from '../../assets/logo.png'
import wailsConfig from '../../../../wails.json'

const DONATE_LINKS = [
  { label: 'GitHub', url: 'https://github.com/sponsors/rnons' },
  { label: 'Liberapay', url: 'https://liberapay.com/rnons' },
  { label: 'PayPal', url: 'https://paypal.me/nonbili' },
  { label: 'Stripe', url: 'https://donate.stripe.com/dRm8wP0PJ94K4F9a9P48003' },
]

const SOURCE_URL = 'https://github.com/nonbili/meron'

export function AboutDialog() {
  const { t } = useTranslation()
  const open = useValue(ui$.aboutOpen)

  const onClose = () => ui$.aboutOpen.set(false)

  if (!open) return null

  const productName = wailsConfig.info.productName
  const version = wailsConfig.info.productVersion
  const comments = wailsConfig.info.comments

  return (
    <Dialog
      onClose={onClose}
      backdropClassName="z-[100] bg-black/35"
      className="flex max-w-sm flex-col overflow-hidden shadow-black/20 dark:shadow-black/45"
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

        <div className="mt-5 flex items-center gap-2">
          <Button variant="secondary" size="sm" rightIcon={ExternalLink} onClick={() => openExternal(SOURCE_URL)}>
            {t('about.sourceCode')}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            leftIcon={ScrollText}
            onClick={() => {
              ui$.aboutOpen.set(false)
              ui$.changelogOpen.set(true)
            }}
          >
            {t('changelog.title')}
          </Button>
        </div>

        <UpdateSection />

        <div className="mt-6 w-full rounded-2xl border border-border/70 bg-raised/70 p-4">
          <div className="flex items-center justify-center gap-2 text-xs font-bold text-primary">
            <Heart size={14} className="text-accent" />
            <span>{t('about.supportDevelopment')}</span>
          </div>
          <div className="mt-3 grid grid-cols-2 gap-2">
            {DONATE_LINKS.map((link) => (
              <Button
                key={link.url}
                variant="secondary"
                size="sm"
                rightIcon={ExternalLink}
                className="px-2 text-[0.625rem]"
                onClick={() => openExternal(link.url)}
              >
                {link.label}
              </Button>
            ))}
          </div>
        </div>
      </div>
    </Dialog>
  )
}
